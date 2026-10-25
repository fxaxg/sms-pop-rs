import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";

let server, Otp, LangContext;
before(async () => {
  server = await createServer({ server: { watch: null }, appType: "custom" });
  ({ Otp } = await server.ssrLoadModule("/src/main/sections/Otp.tsx"));
  ({ LangContext } = await server.ssrLoadModule("/src/shared/i18n.ts"));
});
after(async () => { await server?.close(); });

function render({ auto = false, enabled = true, candidate = true, mode = "direct", lang = "zh" } = {}) {
  const config = { otp: {
    enabled, auto_copy: true, auto_fill: auto,
    caret: { enabled: candidate, duration_seconds: 8, watch_seconds: 60, gap: 24 },
    insertion: { mode, type_delay_ms: 60 },
  } };
  return renderToStaticMarkup(createElement(LangContext.Provider, { value: lang },
    createElement(Otp, { config, update() {} })));
}

test("manual selection remains enabled when automatic filling is off", () => {
  const html = render();
  const selects = [...html.matchAll(/<select\b([^>]*)>/g)];
  assert.equal(selects.length, 2);
  assert.doesNotMatch(selects[0][1], /disabled/);
  assert.match(html, /value="candidate" selected/);
  assert.match(html, /自动填入输入框/);
});

test("automatic filling disables manual selection but keeps writing methods available", () => {
  const html = render({ auto: true, candidate: false, mode: "simulate" });
  const selects = [...html.matchAll(/<select\b([^>]*)>/g)];
  assert.match(selects[0][1], /disabled/);
  assert.doesNotMatch(selects[1][1], /disabled/);
  assert.match(html, /value="disabled" selected/); // preserve the previous manual choice
  assert.match(html, /value="simulate" selected/);
  assert.match(html, /逐字间隔/);
  assert.match(html, /监听窗口/);
  assert.match(html, /关闭自动填入后恢复原选择/);
});

test("disabled OTP assist retains choices and disables its controls; English labels render", () => {
  const html = render({ auto: true, enabled: false, lang: "en" });
  assert.match(html, /Automatically fill text fields/);
  assert.match(html, /Writing method/);
  const firstSelect = html.match(/<select\b([^>]*)>/);
  assert.match(firstSelect[1], /disabled/);
  assert.match(html, /<fieldset[^>]*disabled/);
  assert.match(html, /value="candidate" selected/);
});
