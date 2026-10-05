import type { Config } from "../../shared/api";
import { useT } from "../../shared/i18n";
import { NumberField, Row, Section, TextList, Toggle } from "../components";
import { AppRules } from "./AppRules";

interface Props {
  config: Config;
  update: (mutate: (draft: Config) => void) => void;
}

/** 「通知」页：弹窗开关、外观、过滤。 */
export function Notifications({ config, update }: Props) {
  const t = useT();
  const notifications = config.notifications;

  return (
    <>
      <Section title={t.notifications.title} description={t.notifications.desc}>
        <Row label={t.notifications.enabled}>
          <Toggle
            checked={notifications.enabled}
            onChange={(value) => update((d) => void (d.notifications.enabled = value))}
          />
        </Row>
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
      </Section>

      <AppRules config={config} update={update} />
      <Section title={t.notifications.filterTitle} description={t.notifications.filterDesc}>
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
      </Section>
    </>
  );
}
