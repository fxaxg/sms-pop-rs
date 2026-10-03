import type { ReactNode } from "react";

/** 一个设置分区卡片 */
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="section">
      <div className="section-head">
        <h2>{title}</h2>
        {description && <p className="section-desc">{description}</p>}
      </div>
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
  return (
    <div className="row">
      <div className="row-label">
        <span>{label}</span>
        {hint && <small>{hint}</small>}
      </div>
      <div className="row-control">{children}</div>
    </div>
  );
}

/** 开关 */
export function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
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
