import { describe, expect, it } from "vitest";
import {
  formatCost,
  formatRequestTime,
  formatRequestToken,
  formatTokenCount,
  formatTokenDimension,
  formatTotalTokens,
  isLowCacheRate,
  shortConversationId,
} from "./format";
import type { TokenTotals } from "../types";

describe("formatTokenCount", () => {
  it("renders small counts plainly", () => {
    expect(formatTokenCount(0)).toBe("0");
    expect(formatTokenCount(999)).toBe("999");
  });

  it("uses readable K/M/B/T suffixes", () => {
    expect(formatTokenCount(1000)).toBe("1.0K");
    expect(formatTokenCount(12_345)).toBe("12.3K");
    expect(formatTokenCount(999_500)).toBe("999.5K");
    expect(formatTokenCount(12_345_678)).toBe("12.3M");
    expect(formatTokenCount(1_234_567_890)).toBe("1.2B");
    expect(formatTokenCount(1_234_567_890_123)).toBe("1.2T");
  });

  it("rounds to one decimal", () => {
    expect(formatTokenCount(12_349)).toBe("12.3K");
    expect(formatTokenCount(12_351)).toBe("12.4K");
  });

  it("carries over at the thousand boundary", () => {
    expect(formatTokenCount(999_950)).toBe("1.0M");
    expect(formatTokenCount(999_999)).toBe("1.0M");
  });
});

describe("formatTotalTokens", () => {
  it("renders cumulative input+output readably", () => {
    const totals: TokenTotals = {
      input: 12_300_000,
      output: 45_678,
      total: 12_345_678,
      cached: 1_000_000,
      reasoning: 10_000,
    };
    expect(formatTotalTokens(totals)).toBe("12.3M");
  });

  it("renders a dash when there is no token data at all", () => {
    const totals: TokenTotals = { input: 0, output: 0, total: 0, cached: 0, reasoning: 0 };
    expect(formatTotalTokens(totals)).toBe("-");
  });

  it("renders a dash when the old backend omits the field", () => {
    expect(formatTotalTokens(undefined)).toBe("-");
  });
});

describe("formatTokenDimension", () => {
  it("renders a count readably with the same suffixes as totals", () => {
    expect(formatTokenDimension(999)).toBe("999");
    expect(formatTokenDimension(12_345)).toBe("12.3K");
    expect(formatTokenDimension(12_345_678)).toBe("12.3M");
  });

  it("renders a dash when the dimension is zero", () => {
    expect(formatTokenDimension(0)).toBe("-");
  });

  it("renders a dash when the dimension is missing", () => {
    expect(formatTokenDimension(undefined)).toBe("-");
  });
});

describe("formatRequestToken", () => {
  it("renders a count readably with the same suffixes as totals", () => {
    expect(formatRequestToken(999)).toBe("999");
    expect(formatRequestToken(12_345)).toBe("12.3K");
    expect(formatRequestToken(0)).toBe("0");
  });

  it("renders a dash only when the token count is missing", () => {
    expect(formatRequestToken(null)).toBe("-");
    expect(formatRequestToken(undefined)).toBe("-");
  });
});

describe("formatRequestTime", () => {
  it("renders a dash when the timestamp is missing", () => {
    expect(formatRequestTime(undefined)).toBe("-");
    expect(formatRequestTime(null)).toBe("-");
  });

  it("renders a dash for an unparseable timestamp", () => {
    expect(formatRequestTime("not-a-date")).toBe("-");
  });

  it("formats a valid timestamp as local YYYY-MM-DD HH:MM:SS", () => {
    const ts = "2026-08-02T10:00:00Z";
    const d = new Date(ts);
    const p = (n: number) => String(n).padStart(2, "0");
    const expected = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(
      d.getHours(),
    )}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
    expect(formatRequestTime(ts)).toBe(expected);
  });

  it("keeps the seconds component", () => {
    const ts = "2026-08-02T10:04:07Z";
    const d = new Date(ts);
    const p = (n: number) => String(n).padStart(2, "0");
    const expected = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(
      d.getHours(),
    )}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
    expect(formatRequestTime(ts)).toBe(expected);
  });
});

describe("shortConversationId", () => {
  it("keeps short ids as-is", () => {
    expect(shortConversationId("conv-1")).toBe("conv-1");
  });

  it("keeps the unlabeled group name as-is", () => {
    expect(shortConversationId("unlabeled")).toBe("unlabeled");
  });

  it("truncates long ids with an ellipsis", () => {
    expect(shortConversationId("conv-a1b2c3d4e5f6g7h8i9")).toBe("conv-a1b2c3d…");
  });

  it("keeps ids at the boundary length as-is", () => {
    expect(shortConversationId("0123456789abcdef")).toBe("0123456789abcdef");
    expect(shortConversationId("0123456789abcdef1")).toBe("0123456789ab…");
  });
});

describe("formatCost", () => {
  it("renders a dash when the cost is missing", () => {
    expect(formatCost(null)).toBe("-");
    expect(formatCost(undefined)).toBe("-");
  });

  it("renders explicit zero as $0.00", () => {
    expect(formatCost(0)).toBe("$0.00");
  });

  it("renders sub-dollar amounts with four decimal places", () => {
    expect(formatCost(0.0042)).toBe("$0.0042");
    expect(formatCost(0.1)).toBe("$0.1");
  });

  it("renders dollar amounts with two decimals", () => {
    expect(formatCost(1)).toBe("$1.00");
    expect(formatCost(12.34)).toBe("$12.34");
  });

  it("renders large amounts with K/M suffixes", () => {
    expect(formatCost(1234)).toBe("$1.2K");
    expect(formatCost(12_345_678)).toBe("$12.3M");
  });

  it("flags cache rates below 50% as low", () => {
    expect(isLowCacheRate("49.9%")).toBe(true);
    expect(isLowCacheRate("0%")).toBe(true);
    expect(isLowCacheRate("50%")).toBe(false);
    expect(isLowCacheRate("73.5%")).toBe(false);
    expect(isLowCacheRate("100.0%")).toBe(false);
    expect(isLowCacheRate(undefined)).toBe(false);
    expect(isLowCacheRate("-")).toBe(false);
  });
});
