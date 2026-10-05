import { useCallback, useEffect, useRef, useState } from "react";
import {
  caretHide,
  caretInsert,
  caretLayout,
  getCaretOffer,
  getConfig,
  onCaretOffer,
  type CaretOfferTuple,
} from "../shared/api";
import { dict, resolveLang, type Lang } from "../shared/i18n";
import { useSavedTheme } from "../shared/theme";
import { Icon } from "../shared/Icon";

type Phase = "ready" | "inserting" | "done" | "failed";

/**
 * 光标候选条。
 *
 * 流程：挂载/收到新提议 → 取提议 → 渲染并量出自身尺寸 → 报给 Rust 布局
 * （Rust 用 placement::compute 定位置后把窗口显示出来）。
 * 点「填入」→ Rust 写入 → 显示结果 → 收起。
 */
export function CaretApp() {
  useSavedTheme();
  const [offer, setOffer] = useState<CaretOfferTuple | null>(null);
  const [phase, setPhase] = useState<Phase>("ready");
  const [lang, setLang] = useState<Lang>("zh");
  const pillRef = useRef<HTMLDivElement>(null);
  const generationRef = useRef<number | null>(null);
  const refreshVersion = useRef(0);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    getConfig()
      .then((config) => setLang(resolveLang(config.general.language)))
      .catch(() => {});
  }, []);

  const refresh = useCallback(async () => {
    const version = ++refreshVersion.current;
    const next = await getCaretOffer().catch(() => null);
    if (version !== refreshVersion.current) return;
    if (hideTimer.current) clearTimeout(hideTimer.current);
    generationRef.current = next?.[0] ?? null;
    setOffer(next);
    setPhase("ready");
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = onCaretOffer(refresh);
    return () => {
      ++refreshVersion.current;
      generationRef.current = null;
      if (hideTimer.current) clearTimeout(hideTimer.current);
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // Measure untransformed layout size plus all four transparent shadow gutters.
  useEffect(() => {
    if (!offer || !pillRef.current) return;
    const pill = pillRef.current;
    let previous = "";
    const measure = () => {
      const style = getComputedStyle(document.body);
      const width = Math.ceil(pill.offsetWidth + parseFloat(style.paddingLeft) + parseFloat(style.paddingRight));
      const height = Math.ceil(pill.offsetHeight + parseFloat(style.paddingTop) + parseFloat(style.paddingBottom));
      const key = `${width}:${height}`;
      if (key === previous) return;
      previous = key;
      void caretLayout(offer[0], width, height).catch(() => {});
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(pill);
    return () => observer.disconnect();
  }, [offer, phase, lang]);

  if (!offer) return null;

  const t = dict(lang);
  const [generation, code, , sourceHint] = offer;

  const onInsert = async () => {
    if (phase !== "ready") return;
    setPhase("inserting");
    const result = await caretInsert(generation).catch(() => null);
    if (generationRef.current !== generation) return;

    if (result?.[0]) {
      setPhase("done");
      hideTimer.current = setTimeout(() => { void caretHide(generation).catch(() => {}); }, 900);
    } else {
      // 失败详情 Rust 侧已记日志；界面给统一的人话
      setPhase("failed");
      hideTimer.current = setTimeout(() => { void caretHide(generation).catch(() => {}); }, 1800);
    }
  };

  return (
    <div
      ref={pillRef}
      className={`pill ${phase}`}
      role="group"
      aria-label={t.caret.defaultSource}
    >
      <span className="pill-identity">
        <Icon name="code" size={17} />
        <span className="pill-source" title={sourceHint ?? t.caret.defaultSource}>{sourceHint ?? t.caret.defaultSource}</span>
      </span>
      <span className="pill-code">{code}</span>
      <button className="pill-action" onClick={onInsert} disabled={phase !== "ready"}
        aria-label={`${t.caret.fill} ${code}`}>
        {/* All labels participate in sizing so asynchronous feedback never shifts the bar. */}
        {["ready", "inserting", "done", "failed"].map((state) => (
          <span key={state} className={`pill-action-label ${phase === state ? "visible" : ""}`} aria-hidden={phase !== state}>
            {state === "ready" ? t.caret.fill : state === "inserting" ? t.caret.filling : state === "done" ? t.caret.done : t.caret.failed}
          </span>
        ))}
      </button>
      <span className="sr-only" role="status">{phase === "ready" ? "" : phase === "inserting" ? t.caret.filling : phase === "done" ? t.caret.done : t.caret.failed}</span>
    </div>
  );
}
