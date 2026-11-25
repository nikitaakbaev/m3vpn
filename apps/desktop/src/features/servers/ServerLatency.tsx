export type PingDisplay = "numbers" | "scale" | "both" | "dots";

function latencyClass(latency: number | null): string {
  if (latency === null) return "is-unknown";
  if (latency < 80) return "is-good";
  if (latency < 200) return "is-ok";
  if (latency < 500) return "is-warn";
  return "is-bad";
}

export function ServerLatency({
  latency,
  display,
}: {
  latency: number | null;
  display: PingDisplay;
}) {
  const cls = latencyClass(latency);
  const label = latency === null ? "—" : `${latency} ms`;
  const percent =
    latency === null ? 0 : Math.max(8, Math.min(100, 100 - Math.min(latency, 600) / 6));

  if (display === "dots") {
    return <span className={`latency-dot ${cls}`} role="img" title={label} aria-label={label} />;
  }
  if (display === "scale") {
    return (
      <span className={`latency-bar ${cls}`} role="img" title={label} aria-label={label}>
        <span style={{ width: `${percent}%` }} />
      </span>
    );
  }
  if (display === "both") {
    return (
      <span className="latency-cell">
        <span className={`latency-bar ${cls}`} aria-hidden="true">
          <span style={{ width: `${percent}%` }} />
        </span>
        <span className={`latency ${cls}`}>{label}</span>
      </span>
    );
  }
  return <span className={`latency ${cls}`}>{label}</span>;
}
