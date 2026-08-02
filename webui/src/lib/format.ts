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

const SHORT_ID_MAX = 16;
const SHORT_ID_KEEP = 12;

export function shortConversationId(id: string): string {
  if (id.length <= SHORT_ID_MAX) {
    return id;
  }
  return `${id.slice(0, SHORT_ID_KEEP)}…`;
}
