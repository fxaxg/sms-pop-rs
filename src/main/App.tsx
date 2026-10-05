import { useEffect, useState } from "react";
import { getConfig, saveConfig, type Config } from "../shared/api";
import { LangContext, resolveLang, useT } from "../shared/i18n";
import { Connection } from "./sections/Connection";
import { Notifications } from "./sections/Notifications";
import { Otp } from "./sections/Otp";
import { General } from "./sections/General";
import { Icon, type IconName } from "../shared/Icon";
import { applyTheme } from "../shared/theme";

type Page = "connection" | "notifications" | "otp" | "general";

const NAV_ICONS: Record<Page, IconName> = {
  connection: "phone",
  notifications: "bell",
  otp: "code",
  general: "settings",
};

export function App() {
  const [page, setPage] = useState<Page>("connection");
  const [config, setConfig] = useState<Config | null>(null);
  const [dirty, setDirty] = useState(false);
  const [savedFlash, setSavedFlash] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    applyTheme(config?.general.theme);
  }, [config?.general.theme]);

  useEffect(() => {
    getConfig()
      .then(setConfig)
      .catch((err) => setError(String(err)));
  }, []);

  /** 以「改草稿」的方式更新配置 */
  const update = (mutate: (draft: Config) => void) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const draft = structuredClone(prev);
      mutate(draft);
      return draft;
    });
    setDirty(true);
  };

  const save = async () => {
    if (!config) return;
    try {
      await saveConfig(config);
      setDirty(false);
      setSavedFlash(true);
      setTimeout(() => setSavedFlash(false), 2000);
    } catch (err) {
      setError(String(err));
    }
  };

  const lang = resolveLang(config?.general.language ?? "auto");

  return (
    <LangContext.Provider value={lang}>
      <Shell
        page={page}
        setPage={setPage}
        config={config}
        update={update}
        dirty={dirty}
        savedFlash={savedFlash}
        error={error}
        save={save}
      />
    </LangContext.Provider>
  );
}

function Shell(props: {
  page: Page;
  setPage: (page: Page) => void;
  config: Config | null;
  update: (mutate: (draft: Config) => void) => void;
  dirty: boolean;
  savedFlash: boolean;
  error: string | null;
  save: () => void;
}) {
  const t = useT();
  const { page, setPage, config, update, dirty, savedFlash, error, save } = props;

  const nav: { key: Page; label: string }[] = [
    { key: "connection", label: t.nav.connection },
    { key: "notifications", label: t.nav.notifications },
    { key: "otp", label: t.nav.otp },
    { key: "general", label: t.nav.general },
  ];

  return (
    <div className="layout">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon"><Icon name="message" size={22} /></span>
          <span className="brand-name">SmsPop</span>
        </div>
        <nav aria-label={t.shell.navigation}>
          {nav.map((item) => (
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
        <div className="sidebar-caption">{t.shell.tagline}</div>
      </aside>

      <main className="content">
        <header className="page-header">
          <h1>{t.nav[page]}</h1>
          <p>{t.shell[page]}</p>
        </header>
        {error && <div className="error-banner">{error}</div>}

        {config ? (
          <>
            {page === "connection" && <Connection />}
            {page === "notifications" && <Notifications config={config} update={update} />}
            {page === "otp" && <Otp config={config} update={update} />}
            {page === "general" && <General config={config} update={update} />}
          </>
        ) : (
          <p className="loading">{t.common.loading}</p>
        )}
      </main>

      {(dirty || savedFlash) && (
        <footer className="save-bar">
          {savedFlash ? (
            <span className="saved-ok">{t.common.saved}</span>
          ) : (
            <>
              <span className="dirty-hint">{t.common.unsaved}</span>
              <button className="btn primary" onClick={save}>
                {t.common.save}
              </button>
            </>
          )}
        </footer>
      )}
    </div>
  );
}
