const UNITS = ["", "K", "M", "B", "T"];

import type { TokenTotals } from "../types";

export function formatTokenCount(count: number): string {
  let scaled = count;
  let unit = 0;
  while (scaled >= 1000 && unit < UNITS.length - 1) {
    scaled /= 1000;
    unit += 1;
  }
  if (unit === 0) {
    return String(count);
  }
  const rounded = Math.round(scaled * 10) / 10;
  if (rounded >= 1000 && unit < UNITS.length - 1) {
    scaled = rounded / 1000;
    unit += 1;
  } else {
    scaled = rounded;
  }
  return `${scaled.toFixed(1)}${UNITS[unit]}`;
}

export function formatTotalTokens(total: TokenTotals | undefined): string {
  if (!total || total.total === 0) {
    return "-";
  }
  return formatTokenCount(total.total);
}

export function formatTokenDimension(count: number | undefined): string {
  if (!count) {
    return "-";
  }
  return formatTokenCount(count);
}

export function formatRequestToken(count: number | null | undefined): string {
  if (count == null) {
    return "-";
  }
  return formatTokenCount(count);
}

export function formatRequestTime(ts?: string | null): string {
  if (!ts) {
    return "-";
  }
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) {
    return "-";
  }
  return d.toLocaleTimeString("en-GB", { hour12: false });
}

const SHORT_ID_MAX = 16;
const SHORT_ID_KEEP = 12;

export function shortConversationId(id: string): string {
  if (id.length <= SHORT_ID_MAX) {
    return id;
  }
  return `${id.slice(0, SHORT_ID_KEEP)}…`;
}
