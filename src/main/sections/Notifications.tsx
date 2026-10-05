import { useEffect, useState } from "react";
import { sendTestNotification, type Config } from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Disclosure, NumberField, Row, Section, TextList, Toggle } from "../components";
import { AppRules } from "./AppRules";

interface Props {
  config: Config;
  update: (mutate: (draft: Config) => void) => void;
}

/** 「通知」页：弹窗开关、外观、过滤。 */
export function Notifications({ config, update }: Props) {
  const t = useT();
  const notifications = config.notifications;
  const [countdown, setCountdown] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (countdown === null) return;
    const timer = setTimeout(() => setCountdown(countdown === 0 ? null : countdown - 1), countdown === 0 ? 1500 : 1000);
    return () => clearTimeout(timer);
  }, [countdown]);
  const filterCount = Object.values(notifications.filter).reduce((total, entries) => total + entries.length, 0);

  return (
    <>
      <Section title={t.ui.popups} description={t.ui.popupHint} action={
          <Toggle
            checked={notifications.enabled}
            onChange={(value) => update((d) => void (d.notifications.enabled = value))}
          />
        }>
        <Row label={t.notifications.duration} hint={t.notifications.durationHint}>
          <NumberField
            value={notifications.popup.duration_seconds}
            min={2}
            max={60}
            suffix={t.common.seconds}
            onChange={(value) =>
              update((d) => void (d.notifications.popup.duration_seconds = value))
            }
          />
        </Row>
        <Row label={t.notifications.maxVisible} hint={t.notifications.maxVisibleHint}>
          <NumberField
            value={notifications.popup.max_visible}
            min={1}
            max={10}
            onChange={(value) =>
              update((d) => void (d.notifications.popup.max_visible = value))
            }
          />
        </Row>
      <div className="local-test">
        <div><strong>{t.connection.testTitle}</strong><p>{t.connection.testDesc}</p>{error && <p className="bad-text" role="alert">{error}</p>}</div>
        <button className="btn" disabled={countdown !== null} onClick={async () => { setError(null); setCountdown(4); try { await sendTestNotification(); } catch (err) { setError(String(err)); setCountdown(null); } }}>{countdown === null ? t.connection.sendTest : countdown > 0 ? t.connection.sending(countdown) : t.connection.sent}</button>
      </div>
      </Section>

      <AppRules config={config} update={update} />
      <Disclosure title={t.notifications.filterTitle} hint={filterCount ? t.ui.configured(filterCount) : t.ui.unset}>
        <p className="connection-help">{t.ui.filterNote}</p>
        <p className="connection-help">{t.notifications.filterDesc}</p>
        <Row label={t.notifications.excludeApps} hint={t.notifications.excludeAppsHint}>
          <TextList
            value={notifications.filter.exclude_apps}
            placeholder={"com.tencent.xin"}
            onChange={(value) => update((d) => void (d.notifications.filter.exclude_apps = value))}
          />
        </Row>
        <Row label={t.notifications.includeApps} hint={t.notifications.includeAppsHint}>
          <TextList
            value={notifications.filter.include_apps}
            placeholder={"com.apple.MobileSMS"}
            onChange={(value) => update((d) => void (d.notifications.filter.include_apps = value))}
          />
        </Row>
        <Row label={t.notifications.excludeKeywords} hint={t.notifications.excludeKeywordsHint}>
          <TextList
            value={notifications.filter.exclude_keywords}
            placeholder={t.notifications.excludeKeywords}
            onChange={(value) =>
              update((d) => void (d.notifications.filter.exclude_keywords = value))
            }
          />
        </Row>
        <Row label={t.notifications.includeKeywords} hint={t.notifications.includeKeywordsHint}>
          <TextList
            value={notifications.filter.include_keywords}
            onChange={(value) =>
              update((d) => void (d.notifications.filter.include_keywords = value))
            }
          />
        </Row>
      </Disclosure>
    </>
  );
}
