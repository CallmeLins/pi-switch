export type StatsRange = "today" | "last24h" | "last7d" | "custom";

const HOUR_MS = 3600 * 1000;

function dateFromParts(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function dayAfter(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1);
}

export function todayString(now = Date.now()): string {
  const d = new Date(now);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate(),
  ).padStart(2, "0")}`;
}

/**
 * Resolve a stats window into epoch-millis bounds (left-closed, right-open).
 * "today" means the local calendar day, 24h/7d are rolling windows ending at
 * now, and custom spans start-day midnight to end-day 24:00 (inclusive day).
 * `now` is injectable for deterministic tests.
 */
export function computeStatsWindow(
  range: StatsRange,
  from: string | null,
  to: string | null,
  now = Date.now(),
): { from: number; to: number } {
  switch (range) {
    case "today": {
      const start = new Date(now);
      start.setHours(0, 0, 0, 0);
      return { from: start.getTime(), to: now };
    }
    case "last24h":
      return { from: now - 24 * HOUR_MS, to: now };
    case "last7d":
      return { from: now - 7 * 24 * HOUR_MS, to: now };
    case "custom": {
      if (!from || !to) {
        throw new Error("custom window requires from and to dates");
      }
      const start = dateFromParts(from);
      return { from: start.getTime(), to: dayAfter(dateFromParts(to)).getTime() };
    }
  }
}
