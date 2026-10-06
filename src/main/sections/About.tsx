import { useEffect, useState } from "react";
import { getSettingsMeta, openProjectLink, type ProjectLink } from "../../shared/api";
import { Icon } from "../../shared/Icon";
import { useT } from "../../shared/i18n";
import appIcon from "../../shared/app-icon.svg";
import { SoftwareUpdate } from "./SoftwareUpdate";

function Badge({ src, label }: { src: string; label: string }) {
  const [failed, setFailed] = useState(false);
  return failed ? <span className="about-badge-fallback">{label}</span>
    : <img className="about-badge" src={src} alt={label} referrerPolicy="no-referrer" onError={() => setFailed(true)} />;
}

export function About({ beforeInstall }: { beforeInstall: () => Promise<void> }) {
  const t = useT();
  const [version, setVersion] = useState("…");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let disposed = false;
    void getSettingsMeta().then((meta) => { if (!disposed) setVersion(meta.version); })
      .catch((err) => { if (!disposed) setError(String(err)); });
    return () => { disposed = true; };
  }, []);
  const open = async (target: ProjectLink) => {
    setError(null);
    try { await openProjectLink(target); } catch (err) { setError(String(err)); }
  };
  return <div className="about-page">
    <section className="about-hero" aria-label="SmsPop">
      <div className="about-identity">
        <img className="about-app-icon" src={appIcon} alt="" />
        <div className="about-hero-content">
          <h2>SmsPop <span className="about-version">v{version}</span></h2>
          <p>{t.about.tagline}</p>
          <div className="about-actions">
            <button className="btn about-primary" onClick={() => void open("repository")}><Icon name="external" size={16} />{t.about.repository}</button>
            <button className="btn about-link" onClick={() => void open("issues")}>{t.about.feedback}<Icon name="external" size={14} /></button>
          </div>
        </div>
      </div>
      <div className="about-support"><p className="about-community-note"><Icon name="star" size={14} />{t.about.starHint}</p>
        <button className="about-stars" aria-label={t.about.starAction} onClick={() => void open("repository")}>
          <Badge src="https://img.shields.io/github/stars/fxaxg/sms-pop-rs?style=flat-square&label=Stars&color=737373" label={t.about.starsBadge} />
        </button>
      </div>
    </section>
    <section className="about-update" aria-label={t.updater.title}>
      <SoftwareUpdate beforeInstall={beforeInstall} compact />
    </section>
    <section className="about-community" aria-label={t.about.contributors}>
      <div className="about-community-heading"><h2>{t.about.contributors}</h2></div>
      <div className="about-contributors">
        <button className="about-maintainer" onClick={() => void open("author")}>
          <span className="about-avatar">F</span>
          <span><strong>@fxaxg</strong><small>{t.about.maintainer}</small></span>
          <Icon name="external" size={14} />
        </button>
        <button className="btn about-link" onClick={() => void open("contributors")}>{t.about.allContributors}<Icon name="external" size={14} /></button>
      </div>
    </section>
    <footer className="about-footer"><p>{t.about.privacy}</p><span>© SmsPop contributors · MIT License</span></footer>
    {error && <p className="bad-text" role="alert">{error}</p>}
  </div>;
}
