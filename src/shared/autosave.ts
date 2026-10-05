export type SaveStatus = "idle" | "pending" | "saving" | "saved" | "error";

/** Debounces edits and serializes writes; only the newest queued snapshot is kept. */
export class AutoSave<T> {
  private pending: { value: T; revision: number } | null = null;
  private revision = 0;
  private writing = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly write: (value: T) => Promise<unknown>,
    private readonly notify: (status: SaveStatus, error?: string) => void,
    private readonly delay = 400,
  ) {}

  schedule(value: T) {
    this.pending = { value: structuredClone(value), revision: ++this.revision };
    this.notify("pending");
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => { void this.flush(); }, this.delay);
  }

  async flush() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (this.writing || !this.pending) return;
    this.writing = true;
    try {
      while (this.pending) {
        const snapshot = this.pending;
        this.pending = null;
        this.notify("saving");
        try {
          await this.write(snapshot.value);
        } catch (error) {
          // A newer edit supersedes a failed snapshot, but must still be saved.
          if (this.pending) continue;
          this.pending = snapshot;
          if (this.timer) clearTimeout(this.timer);
          this.timer = null;
          this.notify("error", String(error));
          return;
        }
        if (snapshot.revision === this.revision) this.notify("saved");
      }
    } finally {
      this.writing = false;
    }
  }
}
