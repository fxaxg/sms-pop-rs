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

/** 「连接」页：链路状态 + 新手引导。 */
export function Connection() {
  const t = useT();
  const [link, setLink] = useState<LinkStatePayload | null>(null);
  // 测试通知倒计时：点了之后 Rust 会延迟 4 秒发出，按钮同步倒数
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
    // 4→1 每秒倒数；到 0 显示「已发送」停留 1.5 秒再复位
    const timer = setTimeout(
      () => setCountdown((s) => (s ?? 0) - 1),
      countdown === 0 ? 1500 : 1000,
    );
    return () => clearTimeout(timer);
  }, [countdown]);

  const onSendTest = () => {
    if (countdown !== null) return;
    sendTestNotification().catch(() => {});
    setCountdown(4);
  };

  const unsupported = link?.state === "adapter_unsupported";
  const ready = link?.ready ?? false;
  const stateLabel = link
    ? (t.link[link.state as keyof typeof t.link] ?? link.label)
    : "…";

  return (
    <>
      <Section title={t.connection.statusTitle}>
        <div className={`status-card ${ready ? "ok" : ""} ${unsupported ? "bad" : ""}`}>
          <span className="status-dot" />
          <div>
            <strong>{stateLabel}</strong>
            {link?.detail && <p className="status-detail">{link.detail}</p>}
          </div>
        </div>
      </Section>

      <Section title={t.connection.guideTitle} description={t.connection.guideDesc}>
        <ol className="guide">
          <li className={unsupported ? "bad" : "done"}>
            <strong>{t.connection.step1}</strong>
            {unsupported ? (
              <p className="bad-text">{t.connection.step1Bad}</p>
            ) : (
              <p>{t.connection.step1Ok}</p>
            )}
            <div className="row-control">
              <button className="btn" onClick={() => openBluetoothSettings()}>
                {t.connection.openBluetooth}
              </button>
            </div>
          </li>
          <li className={ready || (link && link.state !== "stopped") ? "done" : ""}>
            <strong>{t.connection.step2}</strong>
            <p>{t.connection.step2Desc}</p>
          </li>
          <li className={ready ? "done" : ""}>
            <strong>{t.connection.step3}</strong>
            <p>{t.connection.step3Desc}</p>
            <div className="row-control">
              <button
                className="btn primary"
                onClick={onSendTest}
                disabled={countdown !== null}
              >
                {countdown === null
                  ? t.connection.sendTest
                  : countdown > 0
                    ? t.connection.sending(countdown)
                    : t.connection.sent}
              </button>
            </div>
          </li>
        </ol>
      </Section>

      <Section title={t.connection.tipsTitle}>
        <Row label={t.connection.tipsLabel}>
          <span className="hint-text">{t.connection.tipsBody}</span>
        </Row>
      </Section>
    </>
  );
}
