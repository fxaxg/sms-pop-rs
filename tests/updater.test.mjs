import { test } from "node:test";
import assert from "node:assert/strict";
import { UpdateController, updateActions, notesLink } from "../src/shared/updater.ts";

const status = (phase, revision = 1, extra = {}) => ({
  revision, phase, version: null, notes: null, downloaded: 0, total: null,
  error: null, retry_attempt: 0, next_retry_at: null, ...extra,
});
const tick = () => new Promise(resolve => setImmediate(resolve));
function backend(initial = status("idle")) {
  return {
    current: initial, accept: undefined, stopped: false,
    async listen(accept) { this.accept = accept; return () => { this.stopped = true; }; },
    async query() { return this.current; },
    async check() {}, async download() {}, async install() {},
    emit(next) { this.current = next; this.accept(next); },
  };
}
// Bind mocks the same way as the API's standalone functions.
const controller = (mock) => new UpdateController(Object.fromEntries(
  ["listen", "query", "check", "download", "install"].map(key => [key, mock[key].bind(mock)])
));

test("events win over stale queries and automatic discovery never opens a dialog", async () => {
  const mock = backend();
  let release;
  mock.query = () => new Promise(resolve => { release = resolve; });
  const updater = controller(mock);
  const stop = updater.connect();
  await tick();
  mock.emit(status("available", 3, { version: "0.4.0" }));
  release(status("latest", 2));
  await tick();
  assert.equal(updater.snapshot().status.phase, "available");
  assert.equal(updater.snapshot().open, false);
  stop();
  assert.equal(mock.stopped, true);
});

test("manual discovery opens dialog and closed dialogs retain background download progress", async () => {
  const mock = backend();
  mock.check = async () => { mock.current = status("available", 2, { version: "0.4.0", notes: "## Changes" }); };
  let finish;
  mock.download = async () => { await new Promise(resolve => { finish = resolve; }); };
  const updater = controller(mock);
  const stop = updater.connect();
  await tick();
  await updater.check();
  assert.equal(updater.snapshot().open, true);
  const download = updater.download();
  updater.close();
  mock.emit(status("downloading", 3, { version: "0.4.0", downloaded: 512, total: 1024 }));
  assert.equal(updater.snapshot().open, false);
  updater.show();
  assert.equal(updater.snapshot().status.downloaded, 512);
  mock.emit(status("ready", 4, { version: "0.4.0", downloaded: 1024, total: 1024 }));
  finish(); await download;
  assert.equal(updateActions(updater.snapshot().status).install, true);
  stop();
});

test("latest version and failed manual checks do not open dialog; recovery clears error", async () => {
  const mock = backend(status("latest"));
  let fail = false;
  mock.check = async () => { if (fail) throw new Error("offline"); };
  const updater = controller(mock);
  const stop = updater.connect(); await tick();
  await updater.check();
  assert.equal(updater.snapshot().open, false);
  fail = true; await updater.check();
  assert.match(updater.snapshot().error, /offline/);
  assert.equal(updater.snapshot().open, false);
  mock.emit(status("checking", 2));
  mock.emit(status("latest", 3));
  assert.equal(updater.snapshot().error, null);
  stop();
});

test("all update phases select the correct actions and persistent notice", () => {
  const expected = {
    disabled: [false, false, false, false], idle: [true, false, false, false],
    checking: [false, false, false, false], latest: [true, false, false, false],
    available: [true, true, false, true], downloading: [false, false, false, true],
    ready: [false, false, true, true], installing: [false, false, false, true],
    check_error: [true, false, false, false], download_error: [true, true, false, true],
    install_error: [false, false, true, true],
  };
  for (const [phase, values] of Object.entries(expected)) {
    const version = ["available", "downloading", "ready", "installing", "download_error", "install_error"].includes(phase) ? "0.4.0" : null;
    const actions = updateActions(status(phase, 1, { version }));
    assert.deepEqual([actions.check, actions.download, actions.install, actions.notice], values, phase);
  }
  assert.equal(updateActions(status("check_error", 2, { version: "0.4.0" })).download, true);
  assert.equal(updateActions(null).check, false);
  assert.equal(updateActions(status("available", 1, { version: "0.4.0" }), true).download, false);
});

test("installation waits for settings, blocks on save failure, and permits retry", async () => {
  const mock = backend(status("ready", 1, { version: "0.4.0" }));
  const events = [];
  let release;
  mock.install = async () => { events.push("install"); if (events.filter(x => x === "install").length === 1) throw new Error("installer failed"); };
  const updater = controller(mock);
  const stop = updater.connect(); await tick();
  await updater.install(async () => { throw new Error("save failed"); });
  assert.deepEqual(events, []);
  assert.match(updater.snapshot().error, /save failed/);
  const first = updater.install(async () => { await new Promise(resolve => { release = resolve; }); events.push("saved"); });
  await updater.install(async () => events.push("duplicate"));
  assert.deepEqual(events, []);
  release(); await first;
  assert.deepEqual(events, ["saved", "install"]);
  assert.match(updater.snapshot().error, /installer failed/);
  await updater.install(async () => events.push("saved again"));
  assert.deepEqual(events, ["saved", "install", "saved again", "install"]);
  stop();
});

test("subscription completing after unmount is cleaned up", async () => {
  let release;
  let stops = 0;
  const mock = backend();
  mock.listen = async () => { await new Promise(resolve => { release = resolve; }); return () => { stops++; }; };
  const updater = controller(mock);
  const stop = updater.connect(); stop(); release(); await tick();
  assert.equal(stops, 1);
  assert.equal(updater.snapshot().status, null);
});

test("update notes allow only HTTPS links without credentials", () => {
  assert.equal(notesLink("https://github.com/fxaxg/sms-pop-rs/releases"), "https://github.com/fxaxg/sms-pop-rs/releases");
  for (const url of ["javascript:alert(1)", "http://example.com", "file:///C:/x", "weixin://", "/relative", "https://user:password@example.com", "not a URL"]) {
    assert.equal(notesLink(url), null, url);
  }
});
