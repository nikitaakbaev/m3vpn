import type { ReactNode } from "react";

interface FieldProps {
  label: string;
  hint?: string;
  children: ReactNode;
  className?: string;
}

export function Field({ label, hint, children, className = "" }: FieldProps) {
  return (
    <label className={`m3-field ${className}`}>
      <span className="m3-field__label">{label}</span>
      {children}
      {hint ? <span className="m3-field__hint">{hint}</span> : null}
    </label>
  );
}
