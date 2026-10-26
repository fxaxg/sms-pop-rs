import { test } from 'node:test';
import assert from 'node:assert/strict';
import { changedPaths, needsApplicationChecks } from '../scripts/ci-changes.mjs';

test('website and documentation changes skip application checks', () => {
  assert.equal(needsApplicationChecks(['website/.vitepress/theme/Home.vue', 'website/package-lock.json', 'website/guide/otp.md', 'website/development/releases.md', 'README.md', '.github/workflows/website.yml']), false);
});

test('mixed changes, CI changes and unknown files still run application checks', () => {
  for (const path of ['crates/smspop-core/src/lib.rs', 'src/main/App.tsx', 'Cargo.lock', 'package-lock.json', '.github/workflows/ci.yml', 'scripts/ci-changes.mjs', 'AGENTS.md', 'new-build-config.json']) {
    assert.equal(needsApplicationChecks(['website/index.md', path]), true, path);
  }
});

test('manual runs, first pushes and unexpected events run application checks', () => {
  const neverGit = () => { throw new Error('should not invoke git'); };
  assert.equal(changedPaths('workflow_dispatch', {}, neverGit), null);
  assert.equal(changedPaths('push', { before: '0'.repeat(40), after: 'b'.repeat(40) }, neverGit), null);
  assert.equal(changedPaths('unknown', {}, neverGit), null);
});

test('PR comparison uses merge base and includes deleted paths when files move', () => {
  const before = 'a'.repeat(40), after = 'b'.repeat(40);
  const paths = changedPaths('pull_request', { pull_request: { base: { sha: before }, head: { sha: after } } }, (command, args) => {
    assert.equal(command, 'git');
    assert.deepEqual(args, ['diff', '--name-only', '--no-renames', '-z', `${before}...${after}`]);
    return 'src/removed.ts\0website/moved.ts\0';
  });
  assert.equal(needsApplicationChecks(paths), true);
});

test('push comparison checks the full push, including file names containing newlines', () => {
  const before = 'a'.repeat(40), after = 'b'.repeat(40);
  const paths = changedPaths('push', { before, after }, (command, args) => {
    assert.deepEqual(args, ['diff', '--name-only', '--no-renames', '-z', before, after]);
    return 'website/with\nnewline.md\0src/main/App.tsx\0';
  });
  assert.deepEqual(paths, ['website/with\nnewline.md', 'src/main/App.tsx']);
});
