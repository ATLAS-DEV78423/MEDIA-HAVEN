import type { ButtonHTMLAttributes, ReactNode } from "react";

import { Icon, type IconName } from "./Icon";
import styles from "./Controls.module.css";

type Variant = "solid" | "glass" | "quiet";

export const buttonClass = (variant: Variant = "glass", extra = ""): string =>
  [styles.button, styles[variant], extra].filter(Boolean).join(" ");

export function ActionButton({
  children,
  icon,
  variant = "glass",
  className = "",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  icon?: IconName;
  variant?: Variant;
}) {
  return (
    <button
      type="button"
      className={buttonClass(variant, className)}
      data-focusable="true"
      {...rest}
    >
      {icon ? <Icon name={icon} className={styles.glyph} /> : null}
      <span>{children}</span>
    </button>
  );
}

export function Chip({
  children,
  active = false,
  icon,
  onClick,
}: {
  children: ReactNode;
  active?: boolean;
  icon?: IconName;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      className={`${styles.chip} ${active ? styles.chipActive : ""}`}
      onClick={onClick}
      aria-pressed={active}
      data-focusable="true"
    >
      {icon ? <Icon name={icon} className={styles.chipGlyph} /> : null}
      <span>{children}</span>
    </button>
  );
}

export function ProgressBar({
  value,
  large = false,
}: {
  value: number;
  large?: boolean;
}) {
  const clamped = Math.min(1, Math.max(0, value));
  return (
    <span
      className={`${styles.progress} ${large ? styles.progressLarge : ""}`}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamped * 100)}
    >
      <span
        className={styles.progressFill}
        style={{ width: `${clamped * 100}%` }}
      />
    </span>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`${styles.toggle} ${checked ? styles.toggleOn : ""}`}
      onClick={() => onChange(!checked)}
      data-focusable="true"
    >
      <span className={styles.toggleKnob} />
    </button>
  );
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (next: T) => void;
  label: string;
}) {
  return (
    <div className={styles.segmented} role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={`${styles.segment} ${
            option.value === value ? styles.segmentActive : ""
          }`}
          aria-pressed={option.value === value}
          onClick={() => onChange(option.value)}
          data-focusable="true"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
