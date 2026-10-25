import { test } from "node:test";
import assert from "node:assert/strict";
import { PreviewUpdateBackend } from "../src/main/dev/previewUpdater.ts";
import { UpdateController } from "../src/shared/updater.ts";

const flush = async () => { for (let index = 0; index < 8; index++) await Promise.resolve(); };
const advance = async (t, milliseconds) => { t.mock.timers.tick(milliseconds); await flush(); };

test("development preview drives shared notice and dialog without real updater calls", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1800000000000 });
  const backend = new PreviewUpdateBackend();
  const controller = new UpdateController(backend);
  const stop = controller.connect(); await flush();
  assert.equal(controller.snapshot().status.phase, "available");
  assert.equal(controller.snapshot().open, false);
  const check = controller.check();
  await advance(t, 600); await check;
  assert.equal(controller.snapshot().open, true);
  const download = controller.download();
  controller.close();
  for (let step = 1; step <= 10; step++) await advance(t, 600);
  await download;
  assert.equal(controller.snapshot().open, false);
  assert.equal(controller.snapshot().status.phase, "ready");
  assert.equal(controller.snapshot().status.downloaded, controller.snapshot().status.total);
  controller.show();
  const install = controller.install(async () => {});
  await flush(); await advance(t, 600); await install;
  assert.equal(controller.snapshot().status.phase, "install_error");
  assert.match(controller.snapshot().status.error, /installer was not launched/);
  stop(); backend.dispose();
});

test("preview check failure recovers automatically after 30 seconds without opening dialog", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1800000000000 });
  const backend = new PreviewUpdateBackend();
  const controller = new UpdateController(backend);
  const stop = controller.connect(); await flush();
  backend.select("check_error");
  assert.equal(controller.snapshot().status.next_retry_at, 1800000030000);
  await advance(t, 29_999);
  assert.equal(controller.snapshot().status.phase, "check_error");
  await advance(t, 1);
  assert.equal(controller.snapshot().status.phase, "checking");
  await advance(t, 600);
  assert.equal(controller.snapshot().status.phase, "available");
  assert.equal(controller.snapshot().status.next_retry_at, null);
  assert.equal(controller.snapshot().open, false);
  stop(); backend.dispose();
});

test("leaving preview cancels recovery and in-flight simulated downloads", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1800000000000 });
  const backend = new PreviewUpdateBackend();
  backend.select("check_error");
  backend.select("downloading");
  await advance(t, 600);
  assert.equal((await backend.query()).total, null);
  assert.ok((await backend.query()).downloaded > 0);
  backend.dispose(); await flush();
  const revision = (await backend.query()).revision;
  await advance(t, 60_000);
  assert.equal((await backend.query()).revision, revision);
});
