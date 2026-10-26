import { appendFileSync, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Exclude only known non-application paths. Unknown files still run software CI.
export function needsApplicationChecks(paths) {
  return paths.some((path) => !(
    path.startsWith('website/') ||
    ['README.md', 'CONTRIBUTING.md', 'SECURITY.md', 'LICENSE',
      '.github/workflows/website.yml'].includes(path)
  ));
}

export function changedPaths(eventName, event, git = execFileSync) {
  if (eventName === 'workflow_dispatch') return null;
  const pr = event.pull_request;
  const before = pr ? pr.base.sha : event.before;
  const after = pr ? pr.head.sha : event.after;
  // First pushes and unexpected events conservatively run all software checks.
  if (![before, after].every((sha) => typeof sha === 'string' && /^[a-f0-9]{40,64}$/i.test(sha) && !/^0+$/.test(sha))) return null;
  const comparison = pr ? `${before}...${after}` : before;
  const args = ['diff', '--name-only', '--no-renames', '-z', comparison];
  if (!pr) args.push(after);
  // --no-renames includes both old and new paths, so moving app code into docs
  // cannot hide the deleted application path from the checks.
  return git('git', args, { encoding: 'utf8' }).split('\0').filter(Boolean);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  let run = true;
  try {
    const event = JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
    const paths = changedPaths(process.env.GITHUB_EVENT_NAME, event);
    run = paths === null || needsApplicationChecks(paths);
  } catch {
    console.warn('Could not determine changed paths; running software checks.');
  }
  appendFileSync(process.env.GITHUB_OUTPUT, `application=${run}\n`);
  console.log(run ? 'Software checks required.' : 'Only website/documentation changes; skipping software checks.');
}
