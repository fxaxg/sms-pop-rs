import type { Config, InsertMode } from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Disclosure, NumberField, Row, Section, Toggle } from "../components";

interface Props {
  config: Config;
  update: (mutate: (draft: Config) => void) => void;
}

/** 「验证码」页：识别、复制、候选条、填入方式。 */
export function Otp({ config, update }: Props) {
  const t = useT();
  const otp = config.otp;

  return (
    <>
      <Section title={t.ui.detect} action={
          <Toggle
            checked={otp.enabled}
            onChange={(value) => update((d) => void (d.otp.enabled = value))}
          />
        }>
        <Row label={t.otp.autoCopy} hint={t.ui.copyHint}>
          <Toggle
            checked={otp.auto_copy}
            disabled={!otp.enabled}
            onChange={(value) => update((d) => void (d.otp.auto_copy = value))}
          />
        </Row>
      </Section>

      <Section title={t.otp.caretTitle} description={t.ui.caretHint} action={
          <Toggle
            checked={otp.caret.enabled}
            disabled={!otp.enabled}
            onChange={(value) => update((d) => void (d.otp.caret.enabled = value))}
          />
        }>
        <fieldset className="settings-fields" disabled={!otp.enabled || !otp.caret.enabled}>
        <Row label={t.otp.caretDuration}>
          <NumberField
            value={otp.caret.duration_seconds}
            min={2}
            max={60}
            suffix={t.common.seconds}
            onChange={(value) => update((d) => void (d.otp.caret.duration_seconds = value))}
          />
        </Row>
        </fieldset>
      </Section>
      <Disclosure title={t.ui.advanced}>
        <fieldset className="settings-fields" disabled={!otp.enabled || !otp.caret.enabled}>
        <Row label={t.otp.watchSeconds} hint={t.otp.watchSecondsHint}>
          <NumberField
            value={otp.caret.watch_seconds}
            min={5}
            max={600}
            suffix={t.common.seconds}
            onChange={(value) => update((d) => void (d.otp.caret.watch_seconds = value))}
          />
        </Row>
        <Row label={t.otp.gap} hint={t.otp.gapHint}>
          <NumberField
            value={otp.caret.gap}
            min={0}
            max={100}
            suffix="px"
            onChange={(value) => update((d) => void (d.otp.caret.gap = value))}
          />
        </Row>
        <Row label={t.otp.mode} hint={t.otp.modeHint}>
          <select
            className="select"
            value={otp.insertion.mode}
            onChange={(event) =>
              update((d) => void (d.otp.insertion.mode = event.target.value as InsertMode))
            }
          >
            <option value="direct">{t.otp.modeDirect}</option>
            <option value="simulate">{t.otp.modeSimulate}</option>
          </select>
        </Row>
        {otp.insertion.mode === "simulate" && (
          <Row label={t.otp.typeDelay}>
            <NumberField
              value={otp.insertion.type_delay_ms}
              min={0}
              max={1000}
              suffix="ms"
              onChange={(value) => update((d) => void (d.otp.insertion.type_delay_ms = value))}
            />
          </Row>
        )}
        </fieldset>
      </Disclosure>
    </>
  );
}
