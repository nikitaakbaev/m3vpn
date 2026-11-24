interface IconProps {
  name: string;
  size?: number;
  filled?: boolean;
  className?: string;
}

export function Icon({ name, size = 24, filled = false, className = "" }: IconProps) {
  return (
    <span
      aria-hidden="true"
      className={`material-symbols-rounded ${className}`}
      style={{ fontSize: `${size}px`, fontVariationSettings: `'FILL' ${filled ? 1 : 0}` }}
    >
      {name}
    </span>
  );
}
