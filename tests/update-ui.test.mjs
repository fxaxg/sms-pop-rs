import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";

let server;
let UpdateNotes, UpdateNotice, UpdateDialog, SoftwareUpdate;
before(async () => {
  // Load the actual TSX components through the project's existing transform pipeline.
  server = await createServer({ server: { watch: null }, appType: "custom" });
  ({ UpdateNotes } = await server.ssrLoadModule("/src/main/UpdateNotes.tsx"));
  ({ UpdateNotice, UpdateDialog, SoftwareUpdate } = await server.ssrLoadModule("/src/main/sections/SoftwareUpdate.tsx"));
});
after(async () => { await server?.close(); });

const render = (component, props) => renderToStaticMarkup(createElement(component, props));
const updater = (phase, extra = {}) => ({
  status: { phase, revision: 1, version: "0.4.0", notes: "", downloaded: 512, total: 1024, retry_attempt: 0, next_retry_at: null, error: null, ...extra },
  error: null, busy: false, open: true,
  controller: { show() {}, close() {}, check() {}, download() {}, install() {} },
});

test("release notes render headings, lists, code, tables, and safe links", () => {
  const html = render(UpdateNotes, {
    notes: "## Changes\n\n- Fixed reconnect\n\n`code`\n\n| Item | Status |\n| --- | --- |\n| Retry | Done |\n\n[Release](https://github.com/fxaxg/sms-pop-rs/releases)",
    onOpen() {},
  });
  for (const fragment of ["<h2>Changes</h2>", "<li>Fixed reconnect</li>", "<code>code</code>", "<table>", '<a href="https://github.com/fxaxg/sms-pop-rs/releases">Release</a>']) assert.ok(html.includes(fragment), fragment);
});

test("raw HTML, images, and non-HTTPS links cannot execute or load resources", () => {
  const html = render(UpdateNotes, {
    notes: '<script>alert(1)</script>\n\n<img src="https://example.com/tracker" onerror="bad()">\n\n![Screenshot](https://example.com/image.png)\n\n[Local](file:///C:/x) [Script](javascript:bad) [App](weixin://) [Insecure](http://example.com)',
    onOpen() {},
  });
  assert.doesNotMatch(html, /<script|<img|href=|src=|onerror=/);
  assert.match(html, /Screenshot/);
  assert.match(html, /Local/);
});

test("sidebar notice reflects available version, progress, and ready state", () => {
  const available = render(UpdateNotice, { updater: updater("available") });
  assert.match(available, /发现新版本/); assert.match(available, /v0\.4\.0/);
  assert.match(available, /aria-haspopup="dialog"/);
  assert.match(render(UpdateNotice, { updater: updater("downloading") }), /50%/);
  assert.match(render(UpdateNotice, { updater: updater("ready") }), /更新已就绪/);
  assert.equal(render(UpdateNotice, { updater: updater("latest", { version: null }) }), "");
});

test("about keeps summary while dialog owns release notes, progress and actions", () => {
  const update = updater("available", { notes: "Unique release details" });
  const about = render(SoftwareUpdate, { updater: update });
  assert.match(about, /检查更新/); assert.match(about, /查看更新/);
  assert.doesNotMatch(about, /Unique release details|下载更新/);
  const dialog = render(UpdateDialog, { updater: update, beforeInstall: async () => {} });
  assert.match(dialog, /<dialog/); assert.match(dialog, /Unique release details/);
  assert.match(dialog, /下载更新/);
  const ready = render(UpdateDialog, { updater: updater("ready"), beforeInstall: async () => {} });
  assert.match(ready, /安装并重启/);
  assert.doesNotMatch(ready, /是否继续|确认安装/);
  assert.match(render(UpdateDialog, { updater: updater("available"), beforeInstall: async () => {} }), /此版本未提供更新说明/);
});

test("unknown download size stays indeterminate and check errors describe automatic retry", () => {
  const unknown = render(UpdateDialog, { updater: updater("downloading", { total: null }), beforeInstall: async () => {} });
  assert.match(unknown, /<progress/); assert.doesNotMatch(unknown, /<progress[^>]*value=/);
  const failed = render(SoftwareUpdate, { updater: updater("check_error", { version: null, next_retry_at: 1800000000000, retry_attempt: 1 }) });
  assert.match(failed, /自动重试/); assert.match(failed, /检查更新/);
});
