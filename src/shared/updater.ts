import type { UpdateStatus } from "./api";

export function updateActions(status: UpdateStatus | null, busy = false) {
  const phase = status?.phase;
  const working = busy || phase === "checking" || phase === "downloading" || phase === "installing";
  const install = phase === "ready" || phase === "install_error";
  const download = !!status?.version && (phase === "available" || phase === "download_error" || phase === "check_error");
  return {
    working,
    check: !!status && phase !== "disabled" && !working && !install,
    download: download && !working,
    install: install && !working,
    notice: !!status?.version && phase !== "latest" && phase !== "disabled",
  };
}

export function notesLink(url: string): string | null {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" && !parsed.username && !parsed.password ? parsed.href : null;
  } catch { return null; }
}

export interface UpdateBackend {
  listen: (accept: (status: UpdateStatus) => void) => Promise<() => void>;
  query: () => Promise<UpdateStatus>;
  check: () => Promise<void>;
  download: () => Promise<void>;
  install: () => Promise<void>;
}

/** Shared across pages and dialog lifetimes; backend events never open the dialog. */
export class UpdateController {
  private value = { status: null as UpdateStatus | null, error: null as string | null, busy: false, open: false };
  private listeners = new Set<() => void>();
  constructor(private backend: UpdateBackend) {}
  snapshot = () => this.value;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };
  private patch(next: Partial<typeof this.value>) {
    this.value = { ...this.value, ...next };
    this.listeners.forEach((listener) => listener());
  }
  private accept = (status: UpdateStatus) => {
    if (!this.value.status || status.revision >= this.value.status.revision) this.patch({ status, error: null });
  };
  connect() {
    let disposed = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await this.backend.listen((status) => { if (!disposed) this.accept(status); });
        if (disposed) { unlisten(); return; }
        stop = unlisten;
        const status = await this.backend.query();
        if (!disposed) this.accept(status);
      } catch (error) { if (!disposed) this.patch({ error: String(error) }); }
    })();
    return () => { disposed = true; stop?.(); };
  }
  show = () => this.patch({ open: true });
  close = () => this.patch({ open: false });
  private async run(action: () => Promise<void>) {
    if (this.value.busy) return;
    this.patch({ busy: true, error: null });
    try { await action(); }
    catch (error) { this.patch({ error: String(error) }); }
    finally { this.patch({ busy: false }); }
  }
  check = () => this.run(async () => {
    await this.backend.check();
    // Query after the command; event delivery may lag behind command completion.
    this.accept(await this.backend.query());
    if (this.value.status?.phase === "available") this.show();
  });
  download = () => this.run(this.backend.download);
  install = (beforeInstall: () => Promise<void>) => this.run(async () => {
    await beforeInstall();
    await this.backend.install();
  });
}
