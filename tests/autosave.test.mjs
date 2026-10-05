import { test } from "node:test";
import assert from "node:assert/strict";
import { AutoSave } from "../src/shared/autosave.ts";

test("rapid edits coalesce and do not mutate saved snapshots", async () => {
  const writes = [];
  const saver = new AutoSave(async (value) => writes.push(value), () => {}, 10);
  saver.schedule({ value: 1 });
  const latest = { value: 2 };
  saver.schedule(latest);
  latest.value = 3;
  await new Promise((resolve) => setTimeout(resolve, 40));
  assert.deepEqual(writes, [{ value: 2 }]);
});

test("writes are serialized and only the latest queued edit is written", async () => {
  const writes = [];
  let release;
  const saver = new AutoSave(async (value) => {
    writes.push(value);
    if (value === 1) await new Promise((resolve) => { release = resolve; });
  }, () => {}, 1000);
  saver.schedule(1);
  const saving = saver.flush();
  saver.schedule(2);
  saver.schedule(3);
  await saver.flush();
  assert.deepEqual(writes, [1]);
  release();
  await saving;
  assert.deepEqual(writes, [1, 3]);
  await saver.flush();
});

test("a failed snapshot is retained for explicit retry", async () => {
  let fail = true;
  const statuses = [];
  const writes = [];
  const saver = new AutoSave(async (value) => {
    if (fail) throw new Error("disk full");
    writes.push(value);
  }, (status) => statuses.push(status), 1000);
  saver.schedule(42);
  await saver.flush();
  assert.equal(statuses.at(-1), "error");
  fail = false;
  await saver.flush();
  assert.deepEqual(writes, [42]);
  assert.equal(statuses.at(-1), "saved");
});

test("failure of an older write does not discard a newer edit", async () => {
  let reject;
  const writes = [];
  const statuses = [];
  const saver = new AutoSave(async (value) => {
    if (value === 1) await new Promise((_, rejectPromise) => { reject = rejectPromise; });
    writes.push(value);
  }, (status) => statuses.push(status), 1000);
  saver.schedule(1);
  const saving = saver.flush();
  saver.schedule(2);
  reject(new Error("temporary failure"));
  await saving;
  assert.deepEqual(writes, [2]);
  assert.equal(statuses.at(-1), "saved");
  await saver.flush();
});
