import type { Config, InsertMode } from "../../shared/api";
import { NumberField, Row, Section, Toggle } from "../components";

interface Props {
  config: Config;
  update: (mutate: (draft: Config) => void) => void;
}

/** 「验证码」页：识别、复制、候选条、填入方式。 */
export function Otp({ config, update }: Props) {
  const otp = config.otp;

  return (
    <>
      <Section
        title="验证码增强"
        description="自动识别通知里的验证码。绝不自动填入 —— 只有你点候选条，验证码才会写进输入框。"
      >
        <Row label="启用验证码增强">
          <Toggle
            checked={otp.enabled}
            onChange={(value) => update((d) => void (d.otp.enabled = value))}
          />
        </Row>
        <Row label="自动复制到剪贴板" hint="收到验证码就复制，随时可以手动粘贴">
          <Toggle
            checked={otp.auto_copy}
            onChange={(value) => update((d) => void (d.otp.auto_copy = value))}
          />
        </Row>
      </Section>

      <Section
        title="光标候选条"
        description="收到验证码时，如果光标正好在输入框里，光标旁会出现「填入」按钮。"
      >
        <Row label="启用光标候选条">
          <Toggle
            checked={otp.caret.enabled}
            onChange={(value) => update((d) => void (d.otp.caret.enabled = value))}
          />
        </Row>
        <Row label="停留时长">
          <NumberField
            value={otp.caret.duration_seconds}
            min={2}
            max={60}
            suffix="秒"
            onChange={(value) => update((d) => void (d.otp.caret.duration_seconds = value))}
          />
        </Row>
        <Row label="监听窗口" hint="收到验证码后，这段时间内点进输入框仍会弹出候选条">
          <NumberField
            value={otp.caret.watch_seconds}
            min={5}
            max={600}
            suffix="秒"
            onChange={(value) => update((d) => void (d.otp.caret.watch_seconds = value))}
          />
        </Row>
        <Row label="与光标间距" hint="默认 24，给中文输入法的候选窗留位置">
          <NumberField
            value={otp.caret.gap}
            min={0}
            max={100}
            suffix="px"
            onChange={(value) => update((d) => void (d.otp.caret.gap = value))}
          />
        </Row>
      </Section>

      <Section title="填入方式" description="绝大多数情况不用动这里。">
        <Row label="模式" hint="direct 快且不要求焦点；simulate 适合顽固的输入框">
          <select
            className="select"
            value={otp.insertion.mode}
            onChange={(event) =>
              update((d) => void (d.otp.insertion.mode = event.target.value as InsertMode))
            }
          >
            <option value="direct">direct — 直接写入</option>
            <option value="simulate">simulate — 模拟键盘</option>
          </select>
        </Row>
        {otp.insertion.mode === "simulate" && (
          <Row label="逐字间隔">
            <NumberField
              value={otp.insertion.type_delay_ms}
              min={0}
              max={1000}
              suffix="ms"
              onChange={(value) => update((d) => void (d.otp.insertion.type_delay_ms = value))}
            />
          </Row>
        )}
      </Section>
    </>
  );
}
