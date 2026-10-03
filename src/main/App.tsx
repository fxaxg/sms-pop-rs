import { useEffect, useState } from "react";
import { getConfig, saveConfig, type Config } from "../shared/api";
import { Connection } from "./sections/Connection";
import { Notifications } from "./sections/Notifications";
import { Otp } from "./sections/Otp";
import { General } from "./sections/General";

type Page = "connection" | "notifications" | "otp" | "general";

const NAV: { key: Page; label: string; icon: string }[] = [
  { key: "connection", label: "连接", icon: "📱" },
  { key: "notifications", label: "通知", icon: "🔔" },
  { key: "otp", label: "验证码", icon: "🔢" },
  { key: "general", label: "通用", icon: "⚙️" },
];

export function App() {
  const [page, setPage] = useState<Page>("connection");
  const [config, setConfig] = useState<Config | null>(null);
  const [dirty, setDirty] = useState(false);
  const [savedFlash, setSavedFlash] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <div className="layout">
      <aside className="sidebar">
        <div className="brand">
          <svg className="brand-icon" width="22" height="22" viewBox="0 0 24 24" aria-hidden>
            <rect x="2" y="4" width="20" height="15" rx="5" fill="var(--accent)" />
            <path d="M8 19 L6 23 L12 19 Z" fill="var(--accent)" />
            <circle cx="8" cy="11.5" r="1.6" fill="#fff" />
            <circle cx="12" cy="11.5" r="1.6" fill="#fff" />
            <circle cx="16" cy="11.5" r="1.6" fill="#fff" />
          </svg>
          <span className="brand-name">SmsPop</span>
        </div>
        <nav>
          {NAV.map((item) => (
            <button
              key={item.key}
              className={`nav-item ${page === item.key ? "active" : ""}`}
              onClick={() => setPage(item.key)}
            >
              <span className="nav-icon">{item.icon}</span>
              {item.label}
            </button>
          ))}
        </nav>
      </aside>

      <main className="content">
        {error && <div className="error-banner">{error}</div>}

        {config ? (
          <>
            {page === "connection" && <Connection />}
            {page === "notifications" && <Notifications config={config} update={update} />}
            {page === "otp" && <Otp config={config} update={update} />}
            {page === "general" && <General />}
          </>
        ) : (
          <p className="loading">载入配置…</p>
        )}
      </main>

      {(dirty || savedFlash) && (
        <footer className="save-bar">
          {savedFlash ? (
            <span className="saved-ok">✓ 已保存</span>
          ) : (
            <>
              <span className="dirty-hint">有未保存的修改</span>
              <button className="btn primary" onClick={save}>
                保存
              </button>
            </>
          )}
        </footer>
      )}
    </div>
  );
}
