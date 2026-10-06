import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

export function checkVersions(root, tag) {
  const pkg = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
  const config = JSON.parse(readFileSync(resolve(root, "src-tauri/tauri.conf.json"), "utf8"));
  const cargo = readFileSync(resolve(root, "Cargo.toml"), "utf8");
  const workspace = cargo.match(/\[workspace\.package\]([\s\S]*?)(?=\n\[|$)/)?.[1];
  const version = workspace?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version || version !== pkg.version || version !== config.version) throw new Error("Cargo, package.json and Tauri versions must match");
  if (tag !== `v${version}`) throw new Error(`Expected tag v${version}, received ${tag}`);
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("Stable releases require a plain major.minor.patch version");
  const lock = JSON.parse(readFileSync(resolve(root, "package-lock.json"), "utf8"));
  if (lock.version !== version || lock.packages[""].version !== version) throw new Error("Run npm install --package-lock-only to synchronize package-lock.json");
  return version;
}

export function makeManifest({ version, repository, filename, signature, notes, date }) {
  if (!signature.trim()) throw new Error("Missing updater signature");
  return {
    version, notes, pub_date: date,
    platforms: {
      "windows-x86_64": {
        url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(filename)}`,
        signature: signature.trim(),
      },
    },
  };
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  const [command, tag, directory] = process.argv.slice(2);
  const version = checkVersions(process.cwd(), tag);
  if (command === "manifest") {
    const installers = readdirSync(directory).filter((name) => name.endsWith(".exe"));
    if (installers.length !== 1) throw new Error("Expected exactly one NSIS installer");
    const filename = installers[0];
    const manifest = makeManifest({
      version, repository: process.env.GITHUB_REPOSITORY, filename,
      signature: readFileSync(resolve(directory, `${filename}.sig`), "utf8"),
      notes: process.env.RELEASE_NOTES || `SmsPop ${version}`,
      date: new Date().toISOString(),
    });
    writeFileSync(resolve(directory, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  } else if (command !== "check") throw new Error("Expected check or manifest");
  console.log(`Validated release ${tag}`);
}
