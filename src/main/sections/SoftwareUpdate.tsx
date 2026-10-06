import { useEffect, useState } from "react";
import { checkUpdate, downloadUpdate, getUpdateStatus, installUpdate, onUpdateStatus, type UpdateStatus } from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Row } from "../components";

export function SoftwareUpdate({ beforeInstall }: { beforeInstall: () => Promise<void> }) {
  const t = useT().updater;
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const accept = (next: UpdateStatus) => {
      if (!disposed) setStatus((prev) => !prev || next.revision >= prev.revision ? next : prev);
    };
    // 先订阅再查询，revision 防止旧查询覆盖新进度。
    void (async () => {
      try {
        const stop = await onUpdateStatus(accept);
        if (disposed) { stop(); return; }
        unlisten = stop;
        accept(await getUpdateStatus());
      } catch (err) { if (!disposed) setError(String(err)); }
    })();
    return () => { disposed = true; unlisten?.(); };
  }, []);

  const run = async (action: () => Promise<void>) => {
    setBusy(true); setError(null);
    try { await action(); }
    catch (err) { setError(String(err)); }
    finally { setBusy(false); }
  };
  const phase = status?.phase;
  const working = busy || phase === "checking" || phase === "downloading" || phase === "installing";
  const canInstall = phase === "ready" || phase === "install_error";
  const canDownload = phase === "available" || phase === "download_error";
  const size = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return <>
    <Row label={t.title} hint={t.hint}>
      <button className="btn" disabled={!status || phase === "disabled" || working || canInstall}
        onClick={() => void run(checkUpdate)}>{t.check}</button>
    </Row>
    {status && <div className="update-panel" aria-live="polite">
      <p className="hint-text">{t[status.phase]}{status.version && ` · ${status.version}`}</p>
      {status.notes && <p className="update-notes">{status.notes}</p>}
      {phase === "downloading" && <>
        <progress aria-label={t.downloading} max={status.total ?? undefined} value={status.total ? status.downloaded : undefined} />
        <p className="hint-text">{size(status.downloaded)}{status.total ? ` / ${size(status.total)}` : ""}</p>
      </>}
      {canDownload && <button className="btn" disabled={working} onClick={() => void run(downloadUpdate)}>{t.download}</button>}
      {canInstall && !confirm && <button className="btn" disabled={working} onClick={() => setConfirm(true)}>{t.install}</button>}
      {canInstall && confirm && <div>
        <p>{t.confirm}</p>
        <button className="btn" disabled={working} onClick={() => void run(async () => { await beforeInstall(); await installUpdate(); })}>{t.confirmInstall}</button>{" "}
        <button className="btn" disabled={working} onClick={() => setConfirm(false)}>{t.cancel}</button>
      </div>}
    </div>}
    {(error || status?.error) && <p className="bad-text" role="alert">{error || status?.error}</p>}
  </>;
}
