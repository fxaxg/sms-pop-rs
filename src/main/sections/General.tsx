import { useEffect, useState } from "react";
import {
  getSettingsMeta,
  openConfigDir,
  openLogsDir,
  setAutostart,
  type Config,
  type SettingsMeta,
  type ThemeSetting,
} from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Row, Section, Toggle } from "../components";

interface Props {
  config: Config;
  update: (mutate: (draft: Config) => void) => void;
}

/** 「通用」页：自启、语言、文件位置、关于。 */
export function General({ config, update }: Props) {
  const t = useT();
  const [meta, setMeta] = useState<SettingsMeta | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getSettingsMeta().then(setMeta).catch(() => {});
  }, []);

  return (
    <>
      <Section title={t.general.appearanceTitle}>
        <Row label={t.general.theme} hint={t.general.themeHint}>
          <select className="select" aria-label={t.general.theme}
            value={config.general.theme}
            onChange={(event) => update((draft) => {
              draft.general.theme = event.target.value as ThemeSetting;
            })}>
            <option value="auto">{t.general.langAuto}</option>
            <option value="light">{t.general.themeLight}</option>
            <option value="dark">{t.general.themeDark}</option>
          </select>
        </Row>
        <Row label={t.general.language} hint={t.general.languageHint}>
          <select
            className="select"
            value={config.general.language}
            onChange={(event) =>
              update((d) => void (d.general.language = event.target.value))
            }
          >
            <option value="auto">{t.general.langAuto}</option>
            <option value="zh">中文</option>
            <option value="en">English</option>
          </select>
        </Row>
      </Section>
      <Section title={t.general.startupTitle}>
        <Row label={t.general.autostart} hint={t.general.autostartHint}>
          <Toggle checked={meta?.autostart_enabled ?? false} disabled={!meta || busy} onChange={async (value) => {
            setBusy(true); setError(null);
            try { const enabled = await setAutostart(value); setMeta((prev) => prev ? { ...prev, autostart_enabled: enabled } : prev); }
            catch (err) { setError(String(err)); }
            finally { setBusy(false); }
          }} />
        </Row>
        {error && <p className="bad-text" role="alert">{error}</p>}
      </Section>

      <Section title={t.general.filesTitle}>
        <Row label={t.general.configFile} hint={meta?.config_path}>
          <button className="btn" onClick={() => openConfigDir()}>
            {t.general.openDir}
          </button>
        </Row>
        <Row label={t.general.logFile} hint={meta?.log_dir}>
          <button className="btn" onClick={() => openLogsDir()}>
            {t.general.openDir}
          </button>
        </Row>
      </Section>

      <Section title={t.general.aboutTitle}>
        <Row label={t.general.version}>
          <span className="hint-text">{t.general.versionText(meta?.version ?? "…")}</span>
        </Row>
        <Row label={t.general.promise}>
          <span className="hint-text">{t.general.promiseText}</span>
        </Row>
      </Section>
    </>
  );
}
