import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "./Icon";

type Variant = "filled" | "tonal" | "outlined" | "text";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  icon?: string;
  trailingIcon?: string;
  children?: ReactNode;
  block?: boolean;
}

export function Button({
  variant = "filled",
  icon,
  trailingIcon,
  children,
  block = false,
  className = "",
  ...props
}: ButtonProps) {
  return (
    <button
      type="button"
      className={`m3-button m3-button--${variant}${block ? " m3-button--block" : ""} ${className}`}
      {...props}
    >
      {icon ? <Icon name={icon} size={18} /> : null}
      {children ? <span>{children}</span> : null}
      {trailingIcon ? <Icon name={trailingIcon} size={18} /> : null}
    </button>
  );
}
