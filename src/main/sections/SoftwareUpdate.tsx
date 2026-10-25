import { useState } from "react";
import { openUpdateLink } from "../../shared/api";
import { updateActions } from "../../shared/updater";
import { useT } from "../../shared/i18n";
import { Modal, Row } from "../components";
import type { Updater } from "../useUpdater";
import { UpdateNotes } from "../UpdateNotes";
import { UpdatePreview } from "../dev/UpdatePreview";

const size = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;

function UpdateSummary({ updater }: { updater: Updater }) {
  const t = useT().updater;
  const { status, error } = updater;
  return <div className="update-panel" aria-live="polite">
    {status && <p className="hint-text">{t[status.phase]}{status.version && ` · v${status.version}`}</p>}
    {status?.next_retry_at != null && <p className="hint-text">{t.retryAt(new Date(status.next_retry_at).toLocaleTimeString())}</p>}
    {(error || status?.error) && <p className="bad-text" role="alert">{error || status?.error}</p>}
  </div>;
}

export function SoftwareUpdate({ updater }: { updater: Updater }) {
  const t = useT().updater;
  const actions = updateActions(updater.status, updater.busy);
  return <>
    <Row label={t.title}>
      {actions.notice && <button className="btn" onClick={updater.controller.show}>{t.details}</button>}
      <button className="btn" disabled={!actions.check} onClick={() => void updater.controller.check()}>{t.check}</button>
    </Row>
    <UpdateSummary updater={updater} />
    {import.meta.env.DEV && <UpdatePreview updater={updater} />}
  </>;
}

export function UpdateNotice({ updater }: { updater: Updater }) {
  const t = useT().updater;
  const { status, controller } = updater;
  if (!status || !updateActions(status).notice) return null;
  const ready = status.phase === "ready" || status.phase === "install_error";
  const progress = status.total ? ` ${Math.min(100, Math.floor(status.downloaded / status.total * 100))}%` : "";
  const label = ready ? t.noticeReady : status.phase === "downloading" ? t.noticeDownloading + progress
    : status.phase === "installing" ? t.installing : t.available;
  return <button className="update-notice" onClick={controller.show} aria-haspopup="dialog">
    <span className="update-notice-dot" aria-hidden="true" />
    <span><strong>{label}</strong><small>v{status.version} · {t.details}</small>{updater.preview && <small>{t.previewActive}</small>}</span>
  </button>;
}

export function UpdateDialog({ updater, beforeInstall }: { updater: Updater; beforeInstall: () => Promise<void> }) {
  const t = useT().updater;
  const { status, controller } = updater;
  const [confirm, setConfirm] = useState(false);
  const [linkError, setLinkError] = useState<string | null>(null);
  const actions = updateActions(status, updater.busy);
  const openLink = async (url: string) => {
    setLinkError(null);
    try { await openUpdateLink(url); } catch (error) { setLinkError(String(error)); }
  };
  return <Modal title={`${updater.preview ? `[${t.previewActive}] ` : ""}${t.title}${status?.version ? ` · v${status.version}` : ""}`} onClose={controller.close}>
    <UpdateSummary updater={updater} />
    <h3 className="update-notes-title">{t.releaseNotes}</h3>
    <UpdateNotes notes={status?.notes?.trim() || t.noNotes} onOpen={(url) => void openLink(url)} />
    {linkError && <p className="bad-text" role="alert">{linkError}</p>}
    {status?.phase === "downloading" && <div className="update-panel" aria-live="polite">
      <progress aria-label={t.downloading} max={status.total ?? undefined} value={status.total ? status.downloaded : undefined} />
      <p className="hint-text">{size(status.downloaded)}{status.total ? ` / ${size(status.total)}` : ""}</p>
    </div>}
    {confirm && actions.install && <p>{t.confirm}</p>}
    <footer className="modal-actions">
      {actions.check && <button className="btn" onClick={() => void controller.check()}>{t.check}</button>}
      {actions.download && <button className="btn primary" onClick={() => void controller.download()}>{t.download}</button>}
      {actions.install && (!confirm
        ? <button className="btn primary" onClick={() => setConfirm(true)}>{t.install}</button>
        : <><button className="btn" onClick={() => setConfirm(false)}>{t.cancel}</button>
          <button className="btn primary" onClick={() => void controller.install(beforeInstall)}>{t.confirmInstall}</button></>)}
    </footer>
  </Modal>;
}
