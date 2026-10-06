import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { checkVersions, makeManifest } from "../scripts/release.mjs";

test("release versions match the tag", () => {
  const { version } = JSON.parse(readFileSync("package.json", "utf8"));
  assert.equal(checkVersions(process.cwd(), `v${version}`), version);
  assert.throws(() => checkVersions(process.cwd(), "v9.9.9"), /Expected tag/);
});
test("manifest contains signature and correctly encoded Windows installer URL", () => {
  const manifest = makeManifest({ version: "0.2.0", repository: "fxaxg/sms-pop-rs", filename: "SmsPop setup.exe", signature: "signed\n", notes: "Changes", date: "2026-10-06T00:00:00Z" });
  assert.equal(manifest.platforms["windows-x86_64"].signature, "signed");
  assert.match(manifest.platforms["windows-x86_64"].url, /v0\.2\.0\/SmsPop%20setup\.exe$/);
  assert.throws(() => makeManifest({ signature: " " }), /Missing updater signature/);
});
