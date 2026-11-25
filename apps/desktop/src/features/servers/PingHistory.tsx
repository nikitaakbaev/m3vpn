import type { PingSample } from "../../lib/tauri";

interface PingHistoryProps {
  history: PingSample[];
  jitterMs: number | null;
  lossPercent: number;
  showGraph: boolean;
  showLoss: boolean;
}

export function PingHistory({
  history,
  jitterMs,
  lossPercent,
  showGraph,
  showLoss,
}: PingHistoryProps) {
  const points = history
    .filter((sample) => sample.ok && sample.latencyMs !== null)
    .slice(-20)
    .map((sample) => sample.latencyMs as number);
  const hasLoss = showLoss && lossPercent > 0;
  if (!showGraph && !hasLoss) return null;
  if (showGraph && points.length < 2 && !hasLoss) return null;

  const width = 54;
  const height = 16;
  const min = Math.min(...points);
  const max = Math.max(...points);
  const scale = max === min ? height / 2 : height - 3;
  const polyline = points
    .map((value, index) => {
      const x = (index / (points.length - 1)) * width;
      const y =
        max === min
          ? height / 2
          : height - ((value - min) / (max - min)) * scale - 1.5;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  const label = [
    jitterMs !== null ? `jitter ${jitterMs} ms` : null,
    `loss ${lossPercent}%`,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <span className="ping-history" role="img" aria-label={label} title={label}>
      {showGraph && points.length >= 2 ? (
        <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
          <polyline points={polyline} fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      ) : null}
      {hasLoss ? (
        <span className="ping-history__loss">{Math.round(lossPercent)}%</span>
      ) : null}
    </span>
  );
}
