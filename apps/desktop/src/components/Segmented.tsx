import { useRef } from "react";
import { Icon } from "./Icon";

export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  icon?: string;
  disabled?: boolean;
}

interface SegmentedProps<T extends string> {
  value: T;
  options: SegmentOption<T>[];
  onChange: (value: T) => void;
  label?: string;
}

export function Segmented<T extends string>({ value, options, onChange, label }: SegmentedProps<T>) {
  const refs = useRef<Array<HTMLButtonElement | null>>([]);
  const enabled = options
    .map((option, index) => ({ option, index }))
    .filter(({ option }) => !option.disabled);

  const move = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    const current = enabled.findIndex((item) => item.index === index);
    if (current < 0) return;
    let next = current;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      next = (current + 1) % enabled.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      next = (current - 1 + enabled.length) % enabled.length;
    } else if (event.key === "Home") {
      next = 0;
    } else if (event.key === "End") {
      next = enabled.length - 1;
    } else {
      return;
    }
    event.preventDefault();
    const target = enabled[next];
    onChange(target.option.value);
    refs.current[target.index]?.focus();
  };

  return (
    <div className="m3-segmented" role="radiogroup" aria-label={label}>
      {options.map((option, index) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            ref={(element) => {
              refs.current[index] = element;
            }}
            type="button"
            role="radio"
            aria-checked={active}
            tabIndex={(active && !option.disabled) || (!active && enabled[0]?.index === index) ? 0 : -1}
            disabled={option.disabled}
            className={`m3-segmented__item${active ? " is-active" : ""}`}
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => move(event, index)}
          >
            {option.icon ? <Icon name={option.icon} size={17} filled={active} /> : null}
            <span>{option.label}</span>
          </button>
        );
      })}
    </div>
  );
}
