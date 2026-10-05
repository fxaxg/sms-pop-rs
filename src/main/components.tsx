import { useEffect, useId, useRef, type ReactNode } from "react";
import { useT } from "../shared/i18n";

export function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const t = useT();
  const ref = useRef<HTMLDialogElement>(null);
  const id = useId();
  useEffect(() => { const dialog = ref.current; dialog?.showModal(); return () => dialog?.close(); }, []);
  return <dialog ref={ref} className="modal" aria-labelledby={id} onCancel={(event) => { event.preventDefault(); onClose(); }}>
    <header className="modal-head"><h2 id={id}>{title}</h2><button className="btn modal-close" aria-label={t.toast.dismiss} onClick={onClose}>×</button></header>
    <div className="modal-body">{children}</div>
  </dialog>;
}

export function Disclosure({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return <details className="disclosure"><summary><span>{title}</span>{hint && <small>{hint}</small>}</summary><div className="disclosure-body">{children}</div></details>;
}

/** 一个设置分区卡片 */
export function Section({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="section">
      {title && <div className="section-head">
        <div>
        <h2>{title}</h2>
        {description && <p className="section-desc">{description}</p>}
        </div>
        {action && <div className="section-action">{action}</div>}
      </div>}
      <div className="section-body">{children}</div>
    </section>
  );
}

/** 一行：左边标签，右边控件 */
export function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <div className="row">
      <div className="row-label">
        <span id={id}>{label}</span>
        {hint && <small>{hint}</small>}
      </div>
      <div className="row-control" role="group" aria-labelledby={id}>{children}</div>
    </div>
  );
}

/** 开关 */
export function Toggle({
  checked,
  onChange,
  disabled = false,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className={`toggle ${checked ? "on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-knob" />
    </button>
  );
}

/** 数字输入 */
export function NumberField({
  value,
  onChange,
  min,
  max,
  step = 1,
  suffix,
}: {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
}) {
  return (
    <span className="number-field">
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(event) => {
          const next = Number(event.target.value);
          if (!Number.isNaN(next)) onChange(next);
        }}
      />
      {suffix && <span className="suffix">{suffix}</span>}
    </span>
  );
}

/** 字符串列表：一行一条 */
export function TextList({
  value,
  onChange,
  placeholder,
}: {
  value: string[];
  onChange: (value: string[]) => void;
  placeholder?: string;
}) {
  return (
    <textarea
      className="text-list"
      rows={3}
      value={value.join("\n")}
      placeholder={placeholder}
      onChange={(event) =>
        onChange(event.target.value.split("\n").filter((line) => line.trim() !== ""))
      }
      spellCheck={false}
    />
  );
}
