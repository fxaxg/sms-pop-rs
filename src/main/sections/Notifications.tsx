import type { Config } from "../../shared/api";
import { NumberField, Row, Section, TextList, Toggle } from "../components";

interface Props {
  config: Config;
  update: (mutate: (draft: Config) => void) => void;
}

/** 「通知」页：弹窗开关、外观、过滤。 */
export function Notifications({ config, update }: Props) {
  const notifications = config.notifications;

  return (
    <>
      <Section
        title="通知弹出"
        description="把 iPhone 的通知实时弹到屏幕右下角。关掉后只剩验证码通知（如果验证码增强开着）。"
      >
        <Row label="启用通知弹出">
          <Toggle
            checked={notifications.enabled}
            onChange={(value) => update((d) => void (d.notifications.enabled = value))}
          />
        </Row>
        <Row label="停留时长" hint="过了就自动消失">
          <NumberField
            value={notifications.popup.duration_seconds}
            min={2}
            max={60}
            suffix="秒"
            onChange={(value) =>
              update((d) => void (d.notifications.popup.duration_seconds = value))
            }
          />
        </Row>
        <Row label="最多同屏" hint="超出的先关掉最旧的">
          <NumberField
            value={notifications.popup.max_visible}
            min={1}
            max={10}
            suffix="条"
            onChange={(value) =>
              update((d) => void (d.notifications.popup.max_visible = value))
            }
          />
        </Row>
      </Section>

      <Section
        title="过滤"
        description="黑名单优先于白名单。App 填 iOS 的 bundle id（如 com.tencent.xin），一行一条。"
      >
        <Row label="排除的 App" hint="这些 App 的通知永不弹出">
          <TextList
            value={notifications.filter.exclude_apps}
            placeholder={"com.tencent.xin"}
            onChange={(value) => update((d) => void (d.notifications.filter.exclude_apps = value))}
          />
        </Row>
        <Row label="只接受这些 App" hint="留空表示不限制">
          <TextList
            value={notifications.filter.include_apps}
            placeholder={"com.apple.MobileSMS"}
            onChange={(value) => update((d) => void (d.notifications.filter.include_apps = value))}
          />
        </Row>
        <Row label="排除关键词" hint="命中任一关键词就不弹">
          <TextList
            value={notifications.filter.exclude_keywords}
            placeholder={"广告"}
            onChange={(value) =>
              update((d) => void (d.notifications.filter.exclude_keywords = value))
            }
          />
        </Row>
        <Row label="只接受关键词" hint="留空表示不限制">
          <TextList
            value={notifications.filter.include_keywords}
            placeholder={"银行"}
            onChange={(value) =>
              update((d) => void (d.notifications.filter.include_keywords = value))
            }
          />
        </Row>
      </Section>
    </>
  );
}
