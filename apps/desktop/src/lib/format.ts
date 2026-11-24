export interface UserInfo {
  upload?: number;
  download?: number;
  total?: number;
  expire?: number;
}

export function parseUserinfo(raw?: string | null): UserInfo {
  if (!raw) return {};
  const result: UserInfo = {};
  for (const part of raw.split(";")) {
    const [key, value] = part.split("=").map((item) => item.trim());
    if (!key || !value) continue;
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) continue;
    if (key === "upload") result.upload = parsed;
    if (key === "download") result.download = parsed;
    if (key === "total") result.total = parsed;
    if (key === "expire") result.expire = parsed > 32_000_000_000 ? Math.floor(parsed / 1000) : parsed;
  }
  return result;
}

export function formatBytes(bytes?: number): string {
  if (bytes === undefined || !Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 ? Math.round(value) : value.toFixed(value >= 10 ? 1 : 2)} ${units[unit]}`;
}

export function formatDate(unix?: number): string {
  if (!unix) return "—";
  return new Date(unix * 1000).toLocaleDateString("ru-RU", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export function formatUptime(startedAt?: number | null): string {
  if (!startedAt) return "—";
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - startedAt);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  if (minutes > 0) return `${minutes} мин`;
  return `${seconds} с`;
}
