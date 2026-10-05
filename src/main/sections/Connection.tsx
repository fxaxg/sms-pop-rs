import { useEffect, useState } from "react";
import {
  forgetDevice,
  getLinkState,
  listDevices,
  onDevicesChanged,
  onLinkState,
  setDeviceEnabled,
  setPreferredDevice,
  type LinkStatePayload,
  type ManagedDevice,
} from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Section } from "../components";
import { Icon } from "../../shared/Icon";
import { ConnectionGuide } from "./ConnectionGuide";

/** 「连接」页：链路状态 + 新手引导。 */
export function Connection() {
  const t = useT();
  const [link, setLink] = useState<LinkStatePayload | null>(null);
  const [devices, setDevices] = useState<ManagedDevice[]>([]);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  const [showGuide, setShowGuide] = useState(false);

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


  const runDeviceAction = async (action: () => Promise<void>) => {
    setDeviceError(null);
    try {
      await action();
    } catch (error) {
      setDeviceError(String(error));
    }
  };

  const unsupported = link?.state === "adapter_unsupported";
  const failed = unsupported || link?.state === "faulted";
  const ready = link?.ready ?? false;
  const stateLabel = link
    ? link.ready
      ? t.connection.readyTitle
      : (t.link[link.state as keyof typeof t.link] ?? link.label)
    : "…";

  return (
    <>
      <div className={`status-card ${ready ? "ok" : ""} ${failed ? "bad" : ""}`} role="status">
          <span className="status-dot" />
          <div>
            <strong>{stateLabel}</strong>
            <p className="status-detail">{failed && link?.detail ? link.detail : ready ? t.connection.readyHint : link?.awaiting_verification ? t.connection.verificationHint : t.connection.waitingHint}</p>
          </div>
      </div>

      <Section title={t.connection.devicesTitle} description={t.connection.devicesDesc} action={devices.length > 0 ? <button className="btn" onClick={() => setShowGuide(true)}><Icon name="plus" size={15} />{t.connection.addDevice}</button> : undefined}>
        {deviceError && <p className="bad-text">{deviceError}</p>}
        {devices.length === 0 ? (
          <div className="empty-device">
            <span className="device-symbol"><Icon name="phone" size={28} /></span>
            <strong>{t.connection.emptyTitle}</strong>
            <p>{t.connection.emptyDesc}</p>
            <button className="btn primary" onClick={() => setShowGuide(true)}>{t.connection.addDevice}</button>
          </div>
        ) : (
          <div className="device-list">
            {devices.map((device) => (
              <article className={`device-card ${device.current ? "current" : ""}`} key={device.id}>
                <div className="device-main">
                   <div className="device-identity">
                     <span className="device-symbol"><Icon name="phone" size={24} /></span>
                     <div>
                     <strong>{device.name}</strong>
                     <span className="device-id">{device.address_hint}</span>
                     </div>
                  </div>
                  <div className="device-badges">
                    {device.preferred && <span className="device-badge preferred">{t.connection.preferred}</span>}
                    {device.current && <span className="device-badge current">{t.connection.current}</span>}
                     {!device.enabled && <span className="device-badge paused">{t.ui.autoConnectOff}</span>}
                  </div>
                </div>
                <p className="device-state">
                  {device.current
                    ? device.connected
                       ? ready ? t.connection.deviceConnected : t.connection.verificationHint
                      : t.connection.deviceConnecting
                    : device.online
                      ? t.connection.deviceOnline
                      : t.connection.deviceOffline}
                  {device.battery_level !== null && ` · ${t.connection.battery(device.battery_level)}`}
                </p>
                {device.last_connected_at !== null && <p className="device-last-seen">{t.connection.lastConnected} · {new Date(device.last_connected_at).toLocaleString()}</p>}
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

      {failed && <button className="btn" onClick={() => setShowGuide(true)}>{t.connection.diagnostics}</button>}
      {link?.detail && <details className="diagnostic-details"><summary>{t.connection.details}</summary><p className="status-detail">{link.detail}</p></details>}
      {showGuide && <ConnectionGuide onClose={() => setShowGuide(false)} />}
    </>
  );
}
