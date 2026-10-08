import { invoke } from "@tauri-apps/api/core";
import { PlatformContext, usePlatform, type PlatformCapabilities } from "../shared/platform";
import { useEffect, useRef, useState } from "react";
import { getConfig, saveConfig, type Config } from "../shared/api";
import { LangContext, resolveLang, useT } from "../shared/i18n";
import { Connection } from "./sections/Connection";
import { Notifications } from "./sections/Notifications";
import { Otp } from "./sections/Otp";
import { General } from "./sections/General";
import { About } from "./sections/About";
import { Network } from "./sections/Network";
import { Icon, type IconName } from "../shared/Icon";
import { applyTheme } from "../shared/theme";
import { AutoSave, type SaveStatus } from "../shared/autosave";
import { Titlebar } from "./Titlebar";

type Page = "connection" | "network" | "notifications" | "otp" | "general" | "about";

const NAV_ICONS: Record<Page, IconName> = {
  connection: "phone",
  network: "external",
  notifications: "bell",
  otp: "code",
  general: "settings",
  about: "info",
};

export function App() {
  const [platform,setPlatform] = useState<PlatformCapabilities>({os:"unknown",ble:false,caret:false,shortcut:false});
  const [page, setPage] = useState<Page>("connection");
  const [config, setConfig] = useState<Config | null>(null);
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const configRef = useRef<Config | null>(null);
  const mounted = useRef(false);
  const [autosave] = useState(() => new AutoSave<Config>(saveConfig, (status, message) => {
    if (!mounted.current) return;
    setSaveStatus(status);
    setSaveError(message ?? null);
  }));

  useEffect(() => {
    mounted.current = true;
    const flush = () => { void autosave.flush(); };
    const onVisibility = () => { if (document.hidden) flush(); };
    window.addEventListener("blur", flush);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      mounted.current = false;
      window.removeEventListener("blur", flush);
      document.removeEventListener("visibilitychange", onVisibility);
      flush();
    };
  }, [autosave]);

  useEffect(() => {
    applyTheme(config?.general.theme);
  }, [config?.general.theme]);

  useEffect(() => {
    let disposed = false;
    Promise.all([getConfig(),invoke<PlatformCapabilities>("get_platform_capabilities")])
      .then(([loaded,capabilities]) => {
        if (disposed) return;
        setPlatform(capabilities);
        if(capabilities.os === "macos") setPage("network");
        configRef.current = loaded;
        setConfig(loaded);
      })
      .catch((err) => { if (!disposed) setError(String(err)); });
    return () => { disposed = true; };
  }, []);

  /** 以「改草稿」的方式更新配置 */
  const update = (mutate: (draft: Config) => void) => {
    if (!configRef.current) return;
    const draft = structuredClone(configRef.current);
    mutate(draft);
    if (JSON.stringify(draft) === JSON.stringify(configRef.current)) return;
    configRef.current = draft;
    setConfig(draft);
    autosave.schedule(draft);
  };

  const lang = resolveLang(config?.general.language ?? "auto");

  return (
    <PlatformContext.Provider value={platform}>
    <LangContext.Provider value={lang}>
      <Shell
        page={page}
        setPage={setPage}
        config={config}
        update={update}
        saveStatus={saveStatus}
        saveError={saveError}
        error={error}
        retry={() => { void autosave.flush(); }}
        beforeClose={() => autosave.flush()}
        beforeInstall={() => autosave.flushBeforeExit()}
      />
    </LangContext.Provider>
    </PlatformContext.Provider>
  );
}

function Shell(props: {
  page: Page;
  setPage: (page: Page) => void;
  config: Config | null;
  update: (mutate: (draft: Config) => void) => void;
  saveStatus: SaveStatus;
  saveError: string | null;
  error: string | null;
  retry: () => void;
  beforeClose: () => Promise<void>;
  beforeInstall: () => Promise<void>;
}) {
  const t = useT();
  const platform=usePlatform();
  const { page, setPage, config, update, saveStatus, saveError, error, retry, beforeClose } = props;

  const nav: { key: Page; label: string }[] = [
    { key: "connection", label: t.nav.connection },
    { key: "network", label: t.nav.network },
    { key: "notifications", label: t.nav.notifications },
    { key: "otp", label: t.nav.otp },
    { key: "general", label: t.nav.general },
    { key: "about", label: t.nav.about },
  ];

  return (
    <div className="app-shell">
      <Titlebar beforeClose={beforeClose} />
      <div className="layout">
      <aside className="sidebar">
        <nav aria-label={t.shell.navigation}>
          {nav.filter(item=>item.key!=="connection"||platform.ble).map((item) => (
            <button
              key={item.key}
              className={`nav-item ${page === item.key ? "active" : ""}`}
              onClick={() => setPage(item.key)}
              aria-current={page === item.key ? "page" : undefined}
            >
              <Icon name={NAV_ICONS[item.key]} />
              {item.label}
            </button>
          ))}
        </nav>
        <div className="sidebar-caption">
          <div className={`autosave-status ${saveStatus === "error" ? "bad-text" : ""}`} role="status">
            {saveStatus === "idle" ? t.common.autoSave : saveStatus === "saved" ? t.common.saved : saveStatus === "error" ? t.common.saveFailed : t.common.saving}
          </div>
          {t.shell.tagline}
        </div>
      </aside>

      <main className="content">
        <header className="page-header">
          <h1>{t.nav[page]}</h1>
          <p>{t.shell[page]}</p>
        </header>
        {error && <div className="error-banner">{error}</div>}
        {saveError && <div className="error-banner" role="alert">
          <span>{t.common.saveFailed}: {saveError}</span>
          <button className="btn" onClick={retry}>{t.common.retry}</button>
        </div>}

        {config ? (
          <>
            {page === "connection" && <Connection />}
            {page === "network" && <Network />}
            {page === "notifications" && <Notifications config={config} update={update} />}
            {page === "otp" && <Otp config={config} update={update} />}
            {page === "general" && <General config={config} update={update} />}
            {page === "about" && <About beforeInstall={props.beforeInstall} />}
          </>
        ) : (
          <p className="loading">{t.common.loading}</p>
        )}
      </main>
      </div>
    </div>
  );
}
