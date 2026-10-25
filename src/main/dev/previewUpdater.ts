import type { UpdateStatus } from "../../shared/api";
import type { UpdateBackend } from "../../shared/updater";

export type PreviewScenario = "available" | "latest" | "check_error" | "downloading" | "download_error" | "ready" | "install_error";

const notes = `## SmsPop 更新预览 / Update preview

- 修复开机时网络未就绪导致检查更新失败的问题。
- 新增左下角更新提示与后台重试。
- Preview headings, lists, links, tables and code.

| 功能 / Feature | 状态 / Status |
| --- | --- |
| 后台重试 / Retry | 已支持 / Supported |
| 更新提示 / Notice | 已支持 / Supported |

使用 \`npm run tauri dev\` 启动开发模式。

[查看项目 / Project](https://github.com/fxaxg/sms-pop-rs)
`;

/** UI-only backend. Never calls Tauri, downloads files, or launches an installer. */
export class PreviewUpdateBackend implements UpdateBackend {
  private status: UpdateStatus = {
    revision: 0, phase: "available", version: "999.0.0-preview", notes,
    downloaded: 0, total: null, error: null, retry_attempt: 0, next_retry_at: null,
  };
  private listeners = new Set<(status: UpdateStatus) => void>();
  private abort = new AbortController();
  private retry: ReturnType<typeof setTimeout> | undefined;

  private emit(next: Partial<UpdateStatus>) {
    this.status = { ...this.status, ...next, revision: this.status.revision + 1 };
    this.listeners.forEach((listener) => listener(this.status));
  }
  private cancel() {
    this.abort.abort();
    this.abort = new AbortController();
    clearTimeout(this.retry);
  }
  private wait(ms: number, signal: AbortSignal) {
    return new Promise<boolean>((resolve) => {
      if (signal.aborted) { resolve(false); return; }
      const finish = (ok: boolean) => {
        clearTimeout(timer);
        signal.removeEventListener("abort", cancelled);
        resolve(ok);
      };
      const cancelled = () => finish(false);
      const timer = setTimeout(() => finish(true), ms);
      signal.addEventListener("abort", cancelled, { once: true });
    });
  }
  listen = async (accept: (status: UpdateStatus) => void) => {
    this.listeners.add(accept);
    return () => { this.listeners.delete(accept); };
  };
  query = async () => this.status;

  select(scenario: PreviewScenario) {
    this.cancel();
    this.emit({
      phase: scenario, version: scenario === "latest" || scenario === "check_error" ? null : "999.0.0-preview",
      notes: scenario === "latest" || scenario === "check_error" ? null : notes,
      error: null, downloaded: 0, total: null, retry_attempt: 0, next_retry_at: null,
    });
    if (scenario === "check_error") {
      this.emit({ error: "Preview: offline", retry_attempt: 1, next_retry_at: Date.now() + 30_000 });
      // Demonstrate background recovery; backend backoff itself is tested in Rust.
      this.retry = setTimeout(() => { void this.check(); }, 30_000);
    } else if (scenario === "downloading") {
      void this.simulateDownload(false);
    } else if (scenario === "download_error" || scenario === "install_error") {
      this.emit({ error: `Preview: ${scenario}` });
    }
  }
  check = async () => {
    this.cancel();
    const signal = this.abort.signal;
    this.emit({ phase: "checking", error: null, next_retry_at: null });
    if (!await this.wait(600, signal)) return;
    this.emit({ phase: "available", version: "999.0.0-preview", notes, retry_attempt: 0 });
  };
  private async simulateDownload(knownSize: boolean) {
    const signal = this.abort.signal;
    const total = 20 * 1024 * 1024;
    this.emit({ phase: "downloading", error: null, downloaded: 0, total: knownSize ? total : null });
    for (let step = 1; step <= 10; step++) {
      if (!await this.wait(600, signal)) return;
      this.emit({ downloaded: total * step / 10 });
    }
    this.emit({ phase: "ready", total });
  }
  download = async () => {
    this.cancel();
    await this.simulateDownload(true);
  };
  install = async () => {
    this.cancel();
    const signal = this.abort.signal;
    this.emit({ phase: "installing", error: null });
    if (!await this.wait(600, signal)) return;
    // Deliberately end in a recoverable error; the app never exits in preview.
    this.emit({ phase: "install_error", error: "Preview: installer was not launched." });
  };
  dispose() {
    this.cancel();
    this.listeners.clear();
  }
}
