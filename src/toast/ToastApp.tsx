import { useEffect, useState } from "react";
import {
  getToastPayload,
  toastClick,
  toastClose,
  toastResize,
  type ToastPayload,
} from "../shared/api";

/**
 * 右下角通知弹窗。
 *
 * 负载不由事件下发（事件可能早于页面加载），而是页面挂载后主动向 Rust 取。
 * 点击：有验证码 → 复制 + 短暂反馈；无验证码 → 直接关闭。
 */
export function ToastApp() {
  const [payload, setPayload] = useState<ToastPayload | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    getToastPayload().then((payload) => {
      if (!payload) {
        // 没有负载（比如兜底定时器已把它清掉）→ 直接关
        toastClose();
        return;
      }
      setPayload(payload);
    });
  }, []);

  // 渲染完后量出真实高度报给 Rust：它调窗口尺寸 + 重排 + 显示。
  // （内容两行三行都可能，固定高度会裁掉「点击复制」那一行。）
  useEffect(() => {
    if (!payload) return;
    const card = document.querySelector(".toast");
    if (!card) return;
    const height = Math.ceil(card.getBoundingClientRect().height) + 8; // body 上下各 4px 阴影留白
    toastResize(height);
  }, [payload]);

  if (!payload) {
    return null;
  }

  const onClick = async () => {
    if (copied) return;
    const code = await toastClick().catch(() => null);
    if (code) {
      setCopied(true);
      setTimeout(() => toastClose(), 700);
    } else {
      toastClose();
    }
  };

  return (
    <div className="toast" onClick={onClick}>
      <div className="toast-icon">{payload.code ? "🔢" : "💬"}</div>
      <div className="toast-text">
        <div className="toast-origin">{payload.origin}</div>
        <div className="toast-body">{payload.body}</div>
        {payload.code && (
          <div className={`toast-code ${copied ? "copied" : ""}`}>
            {copied ? "✓ 已复制" : `点击复制 ${payload.code}`}
          </div>
        )}
      </div>
      <button
        className="toast-dismiss"
        onClick={(event) => {
          event.stopPropagation();
          toastClose();
        }}
        aria-label="关闭"
      >
        ×
      </button>
      <div
        className="toast-countdown"
        style={{ animationDuration: `${payload.duration_secs}s` }}
      />
    </div>
  );
}
