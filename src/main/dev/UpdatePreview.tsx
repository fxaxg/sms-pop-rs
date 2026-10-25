import { useState } from "react";
import { useT } from "../../shared/i18n";
import type { Updater } from "../useUpdater";
import type { PreviewScenario } from "./previewUpdater";

export function UpdatePreview({ updater }: { updater: Updater }) {
  const t = useT().updater;
  const [error, setError] = useState<string | null>(null);
  if (!updater.startPreview) return null;
  const scenarios: PreviewScenario[] = ["available", "latest", "check_error", "downloading", "download_error", "ready", "install_error"];
  return <details className="update-preview">
    <summary>{t.previewTitle}</summary>
    <p className="hint-text">{t.previewHint}</p>
    {updater.preview ? <>
      <label>{t.previewScenario}{" "}<select className="select" defaultValue="available" disabled={updater.busy}
        onChange={(event) => updater.preview?.select(event.target.value as PreviewScenario)}>
        {scenarios.map((scenario) => <option key={scenario} value={scenario}>{t[scenario]}</option>)}
      </select></label>
      <p className="hint-text">{t.previewRetryHint}</p>
      <button className="btn" onClick={updater.stopPreview}>{t.previewStop}</button>
    </> : <button className="btn" onClick={() => {
      setError(null);
      void updater.startPreview?.().catch((err: unknown) => setError(String(err)));
    }}>{t.previewStart}</button>}
    {error && <p className="bad-text" role="alert">{error}</p>}
  </details>;
}
