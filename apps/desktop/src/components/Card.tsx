import type { HTMLAttributes } from "react";

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  variant?: "default" | "elevated" | "outlined" | "accent";
}

export function Card({ variant = "default", className = "", ...props }: CardProps) {
  return <div className={`m3-card m3-card--${variant} ${className}`} {...props} />;
}
