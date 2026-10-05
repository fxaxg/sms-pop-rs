import { useEffect, useState } from "react";
import {
  forgetDevice,
  getLinkState,
  listDevices,
  onDevicesChanged,
  onLinkState,
  openBluetoothSettings,
  sendTestNotification,
  setDeviceEnabled,
  setPreferredDevice,
  type LinkStatePayload,
  type ManagedDevice,
} from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Section, Row } from "../components";

/** 「连接」页：链路状态 + 新手引导。 */
export function Connection() {
  const t = useT();
  const [link, setLink] = useState<LinkStatePayload | null>(null);
  const [devices, setDevices] = useState<ManagedDevice[]>([]);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  // 测试通知倒计时：点了之后 Rust 会延迟 4 秒发出，按钮同步倒数
  const [countdown, setCountdown] = useState<number | null>(null);

  useEffect(() => {
    getLinkState().then(setLink).catch(() => {});
    listDevices().then(setDevices).catch(() => {});
    const unlisten = onLinkState(setLink);
    const unlistenDevices = onDevicesChanged(setDevices);
    return () => {
      unlisten.then((fn) => fn());
      unlistenDevices.then((fn) => fn());
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

  const runDeviceAction = async (action: () => Promise<void>) => {
    setDeviceError(null);
    try {
      await action();
    } catch (error) {
      setDeviceError(String(error));
    }
  };

  const unsupported = link?.state === "adapter_unsupported";
  const ready = link?.ready ?? false;
  const stateLabel = link
    ? link.ready
      ? t.connection.readyTitle
      : (t.link[link.state as keyof typeof t.link] ?? link.label)
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

      <Section title={t.connection.devicesTitle} description={t.connection.devicesDesc}>
        {deviceError && <p className="bad-text">{deviceError}</p>}
        {devices.length === 0 ? (
          <p className="connection-help">{t.connection.devicesEmpty}</p>
        ) : (
          <div className="device-list">
            {devices.map((device) => (
              <article className={`device-card ${device.current ? "current" : ""}`} key={device.id}>
                <div className="device-main">
                  <div>
                    <strong>{device.name}</strong>
                    <span className="device-id">{device.address_hint}</span>
                  </div>
                  <div className="device-badges">
                    {device.preferred && <span className="device-badge preferred">{t.connection.preferred}</span>}
                    {device.current && <span className="device-badge current">{t.connection.current}</span>}
                    {!device.enabled && <span className="device-badge paused">{t.connection.paused}</span>}
                  </div>
                </div>
                <p className="device-state">
                  {device.current
                    ? device.connected
                      ? t.connection.deviceConnected
                      : t.connection.deviceConnecting
                    : device.online
                      ? t.connection.deviceOnline
                      : t.connection.deviceOffline}
                  {device.battery_level !== null && ` · ${t.connection.battery(device.battery_level)}`}
                </p>
                <div className="device-actions">
                  {!device.preferred && (
                    <button className="btn" onClick={() => runDeviceAction(() => setPreferredDevice(device.id))}>
                      {t.connection.makePreferred}
                    </button>
                  )}
                  <button
                    className="btn"
                    onClick={() => runDeviceAction(() => setDeviceEnabled(device.id, !device.enabled))}
                  >
                    {device.enabled ? t.connection.pauseDevice : t.connection.enableDevice}
                  </button>
                  <button
                    className="btn danger"
                    disabled={device.current}
                    onClick={() => {
                      if (confirm(t.connection.forgetConfirm)) {
                        runDeviceAction(() => forgetDevice(device.id));
                      }
                    }}
                  >
                    {t.connection.forgetDevice}
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
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
