import { useEffect, useState } from "react";
import {
  getLinkState,
  onLinkState,
  openBluetoothSettings,
  sendTestNotification,
  type LinkStatePayload,
} from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Section, Row } from "../components";

/** 「连接」页：只向用户展示当前结果和下一步操作。 */
export function Connection() {
  const t = useT();
  const [link, setLink] = useState<LinkStatePayload | null>(null);
  const [countdown, setCountdown] = useState<number | null>(null);

  useEffect(() => {
    getLinkState().then(setLink).catch(() => {});
    const unlisten = onLinkState(setLink);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (countdown === null) return;
    if (countdown < 0) {
      setCountdown(null);
      return;
    }
    const timer = setTimeout(
      () => setCountdown((value) => (value ?? 0) - 1),
      countdown === 0 ? 1500 : 1000,
    );
    return () => clearTimeout(timer);
  }, [countdown]);

  const state = link?.state ?? "stopped";
  const bad = state === "faulted" || state === "adapter_unsupported";
  const status = link?.ready
    ? { title: t.connection.readyTitle, body: t.connection.readyBody }
    : link?.awaiting_verification
      ? { title: t.connection.verifyingTitle, body: t.connection.verifyingBody }
      : state === "connected"
        ? { title: t.connection.connectingTitle, body: t.connection.connectingBody }
        : state === "faulted"
          ? { title: t.connection.errorTitle, body: t.connection.errorBody }
          : state === "adapter_unsupported"
            ? { title: t.connection.unsupportedTitle, body: t.connection.step1Bad }
            : state === "stopped"
              ? { title: t.connection.stoppedTitle, body: t.connection.stoppedBody }
              : { title: t.connection.waitingTitle, body: t.connection.waitingBody };

  const onSendTest = () => {
    if (countdown !== null) return;
    sendTestNotification().catch(() => {});
    setCountdown(4);
  };

  return (
    <>
      <Section title={t.connection.statusTitle}>
        <div className={`status-card status-card-main ${link?.ready ? "ok" : ""} ${bad ? "bad" : ""}`}>
          <span className="status-dot" />
          <div className="status-copy">
            <strong>{status.title}</strong>
            <p>{status.body}</p>
          </div>
        </div>
        <div className="connection-actions">
          <button className="btn primary" onClick={() => openBluetoothSettings()}>
            {t.connection.openBluetooth}
          </button>
        </div>
      </Section>

      {!link?.ready && (
        <Section title={t.connection.guideTitle} description={t.connection.guideDesc}>
          <ol className="guide">
            <li className={state !== "adapter_unsupported" ? "done" : "bad"}>
              <strong>{t.connection.step1}</strong>
              <p>{state === "adapter_unsupported" ? t.connection.step1Bad : t.connection.step1Ok}</p>
            </li>
            <li className={link?.awaiting_verification || state === "connected" ? "done" : ""}>
              <strong>{t.connection.step2}</strong>
              <p>{t.connection.step2Desc}</p>
            </li>
            <li className={link?.ready ? "done" : ""}>
              <strong>{t.connection.step3}</strong>
              <p>{t.connection.step3Desc}</p>
            </li>
          </ol>
        </Section>
      )}

      <Section title={t.connection.tipsTitle}>
        <Row label={t.connection.tipsLabel}>
          <span className="hint-text">{t.connection.tipsBody}</span>
        </Row>
      </Section>

      <Section title={t.connection.repairTitle}>
        <p className="connection-help">{t.connection.repairBody}</p>
        <div className="connection-actions">
          <button className="btn" onClick={() => openBluetoothSettings()}>
            {t.connection.openBluetooth}
          </button>
        </div>
      </Section>

      <Section title={t.connection.sendTest}>
        <p className="connection-help">{t.connection.testHint}</p>
        <div className="connection-actions">
          <button className="btn" onClick={onSendTest} disabled={countdown !== null}>
            {countdown === null
              ? t.connection.sendTest
              : countdown > 0
                ? t.connection.sending(countdown)
                : t.connection.sent}
          </button>
        </div>
      </Section>

      <details className="technical-details">
        <summary>{t.connection.technicalDetails}</summary>
        <p>{link?.detail || t.connection.noTechnicalDetails}</p>
        <code>{link?.state ?? "stopped"}</code>
      </details>
    </>
  );
}
