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

const radios = (html) => [...html.matchAll(/<input\b([^>]*)>/g)]
  .map((match) => match[1]).filter((attributes) => attributes.includes('type="radio"'));
const checkedMode = (html) => radios(html).find((attributes) => attributes.includes("checked="))?.match(/value="([^"]+)"/)?.[1];

test("candidate mode is selected by default and shows its settings", () => {
  const html = render();
  assert.equal(radios(html).length, 3);
  assert.equal(checkedMode(html), "candidate");
  assert.match(html, /role="radiogroup"/);
  assert.match(html, /在输入框旁显示验证码，点击填入/);
  assert.match(html, /停留时长/);
  assert.match(html, /与光标间距/);
  assert.match(html, /监听时长/);
  assert.match(html, /写入机制/);
  assert.doesNotMatch(html, /手动填入入口|自动填入输入框/);
});

test("automatic mode takes precedence for existing settings and hides candidate-only fields", () => {
  const html = render({ auto: true, candidate: true, mode: "simulate" });
  const selects = [...html.matchAll(/<select\b([^>]*)>/g)];
  assert.equal(selects.length, 1); // only the writing mechanism remains a dropdown
  assert.doesNotMatch(selects[0][1], /disabled/);
  assert.equal(checkedMode(html), "automatic");
  assert.match(html, /value="simulate" selected/);
  assert.match(html, /逐字间隔/);
  assert.match(html, /监听时长/);
  assert.match(html, /每条只尝试一次/);
  assert.doesNotMatch(html, /停留时长|与光标间距/);
});

test("off mode hides insertion parameters while keeping recognition and copying controls", () => {
  const html = render({ auto: false, candidate: false });
  assert.equal(checkedMode(html), "off");
  assert.match(html, /仍可识别和复制验证码/);
  assert.match(html, /识别验证码/);
  assert.match(html, /自动复制到剪贴板/);
  assert.doesNotMatch(html, /监听时长|写入机制|停留时长|与光标间距|高级选项/);
});

test("disabled OTP assist retains choices and disables its controls; English labels render", () => {
  const html = render({ auto: true, enabled: false, lang: "en" });
  assert.match(html, /Automatic fill/);
  assert.match(html, /Insertion mechanism/);
  assert.match(html, /<fieldset[^>]*role="radiogroup"[^>]*disabled/);
  assert.equal(checkedMode(html), "automatic");
});
