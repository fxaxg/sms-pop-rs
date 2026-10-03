import { useCallback, useEffect, useRef, useState } from "react";
import {
  caretHide,
  caretInsert,
  caretLayout,
  getCaretOffer,
  onCaretOffer,
  type CaretOfferTuple,
} from "../shared/api";

type Phase = "ready" | "inserting" | "done" | "failed";

/**
 * 光标候选条。
 *
 * 流程：挂载/收到新提议 → 取提议 → 渲染并量出自身尺寸 → 报给 Rust 布局
 * （Rust 用 placement::compute 定位置后把窗口显示出来）。
 * 点「填入」→ Rust 写入 → 显示结果 → 收起。
 */
export function CaretApp() {
  const [offer, setOffer] = useState<CaretOfferTuple | null>(null);
  const [phase, setPhase] = useState<Phase>("ready");
  const [message, setMessage] = useState("");
  const pillRef = useRef<HTMLButtonElement>(null);

  const refresh = useCallback(async () => {
    const next = await getCaretOffer().catch(() => null);
    setOffer(next);
    setPhase("ready");
    setMessage("");
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = onCaretOffer(refresh);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // 渲染完成后把真实尺寸报给 Rust 布局
  // （用 right/bottom 而不是 width/height：把 body 给阴影留的 padding 也算进去）
  useEffect(() => {
    if (!offer || phase !== "ready" || !pillRef.current) return;
    const rect = pillRef.current.getBoundingClientRect();
    caretLayout(offer[0], Math.ceil(rect.right), Math.ceil(rect.bottom));
  }, [offer, phase]);

  if (!offer) return null;

  const [generation, code] = offer;

  const onInsert = async () => {
    if (phase !== "ready") return;
    setPhase("inserting");
    const result = await caretInsert(generation).catch(() => null);

    if (result?.[0]) {
      setPhase("done");
      setMessage("已填入");
      setTimeout(() => caretHide(), 900);
    } else {
      setPhase("failed");
      setMessage(result?.[1] ?? "写入失败，请手动粘贴（已复制到剪贴板）");
      setTimeout(() => caretHide(), 1800);
    }
  };

  return (
    <button
      ref={pillRef}
      className={`pill ${phase}`}
      onClick={onInsert}
      disabled={phase === "inserting"}
    >
      {phase === "ready" && (
        <>
          <span className="pill-action">填入</span>
          <span className="pill-code">{code}</span>
        </>
      )}
      {phase === "inserting" && <span>填入中…</span>}
      {phase === "done" && <span>✓ {message}</span>}
      {phase === "failed" && <span>✗ {message}</span>}
    </button>
  );
}
