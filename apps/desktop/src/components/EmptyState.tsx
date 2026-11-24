import type { ReactNode } from "react";
import { Icon } from "./Icon";

interface EmptyStateProps {
  icon: string;
  title: string;
  text: string;
  action?: ReactNode;
}

export function EmptyState({ icon, title, text, action }: EmptyStateProps) {
  return (
    <div className="empty-state">
      <div className="empty-state__icon">
        <Icon name={icon} size={34} />
      </div>
      <h2>{title}</h2>
      <p>{text}</p>
      {action ? <div className="empty-state__action">{action}</div> : null}
    </div>
  );
}
