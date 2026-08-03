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

export function formatCost(value: number | null | undefined): string {
  if (value == null) {
    return "-";
  }
  if (value === 0) {
    return "$0.00";
  }
  if (value < 1) {
    // Sub-dollar amounts keep four decimal places, trailing zeros trimmed.
    return `$${value.toFixed(4).replace(/\.?0+$/, "")}`;
  }
  if (value < 1000) {
    return `$${value.toFixed(2)}`;
  }
  // Large amounts reuse the token K/M/B/T suffix scaling.
  return `$${formatTokenCount(value)}`;
}

export function formatRequestTime(ts?: string | null): string {
  if (!ts) {
    return "-";
  }
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) {
    return "-";
  }
  const pad2 = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(
    d.getHours(),
  )}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}

const SHORT_ID_MAX = 16;
const SHORT_ID_KEEP = 12;

export function shortConversationId(id: string): string {
  if (id.length <= SHORT_ID_MAX) {
    return id;
  }
  return `${id.slice(0, SHORT_ID_KEEP)}…`;
}
