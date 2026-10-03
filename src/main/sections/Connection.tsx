import { useEffect, useState } from "react";
import {
  getLinkState,
  onLinkState,
  openBluetoothSettings,
  sendTestNotification,
  type LinkStatePayload,
} from "../../shared/api";
import { Section, Row } from "../components";

/** 「连接」页：链路状态 + 新手引导。 */
export function Connection() {
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

  return (
    <>
      <Section title="连接状态">
        <div className={`status-card ${ready ? "ok" : ""} ${unsupported ? "bad" : ""}`}>
          <span className="status-dot" />
          <div>
            <strong>{link?.label ?? "…"}</strong>
            {link?.detail && <p className="status-detail">{link.detail}</p>}
          </div>
        </div>
      </Section>

      <Section
        title="连接引导"
        description="按顺序完成这三步，iPhone 的通知就会出现在这里。"
      >
        <ol className="guide">
          <li className={unsupported ? "bad" : "done"}>
            <strong>确认电脑蓝牙已打开</strong>
            {unsupported ? (
              <p className="bad-text">
                这台电脑的蓝牙适配器不支持 BLE 外设角色，SmsPop 无法在这台电脑上工作。
              </p>
            ) : (
              <p>本机蓝牙就绪。</p>
            )}
            <div className="row-control">
              <button className="btn" onClick={() => openBluetoothSettings()}>
                打开蓝牙设置
              </button>
            </div>
          </li>
          <li className={ready || (link && link.state !== "stopped") ? "done" : ""}>
            <strong>在 iPhone 的蓝牙设置里找到这台电脑并配对</strong>
            <p>
              配对时 iPhone 会弹「是否允许接收你的 iPhone 通知」—— <b>必须点允许</b>，
              否则一条通知都收不到（而且不会有任何报错）。
            </p>
          </li>
          <li className={ready ? "done" : ""}>
            <strong>等待链路就绪</strong>
            <p>
              状态变成「已就绪」后，发条短信试试，或点下面的按钮发一条测试通知
              （4 秒后发出 —— 趁这几秒把光标点进一个输入框，还能顺便看到「填入」候选条）。
            </p>
            <div className="row-control">
              <button
                className="btn primary"
                onClick={onSendTest}
                disabled={countdown !== null}
              >
                {countdown === null
                  ? "发送测试通知"
                  : countdown > 0
                    ? `${countdown} 秒后发出…`
                    : "已发送 ✓"}
              </button>
            </div>
          </li>
        </ol>
      </Section>

      <Section title="提示">
        <Row label="收不到通知？">
          <span className="hint-text">
            去 iPhone：设置 → 蓝牙 → 点这台电脑右边的 ⓘ → 打开「共享系统通知」。
          </span>
        </Row>
      </Section>
    </>
  );
}
