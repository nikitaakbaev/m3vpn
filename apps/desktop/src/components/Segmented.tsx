import { Icon } from "./Icon";

export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  icon?: string;
}

interface SegmentedProps<T extends string> {
  value: T;
  options: SegmentOption<T>[];
  onChange: (value: T) => void;
  label?: string;
}

export function Segmented<T extends string>({ value, options, onChange, label }: SegmentedProps<T>) {
  return (
    <div className="m3-segmented" role="tablist" aria-label={label}>
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="tab"
            aria-selected={active}
            className={`m3-segmented__item${active ? " is-active" : ""}`}
            onClick={() => onChange(option.value)}
          >
            {option.icon ? <Icon name={option.icon} size={17} filled={active} /> : null}
            <span>{option.label}</span>
          </button>
        );
      })}
    </div>
  );
}
