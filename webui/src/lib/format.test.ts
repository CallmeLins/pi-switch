import { describe, expect, it } from "vitest";
import { formatTokenCount, formatTotalTokens, shortConversationId } from "./format";
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
    const totals: TokenTotals = { input: 12_300_000, output: 45_678, total: 12_345_678 };
    expect(formatTotalTokens(totals)).toBe("12.3M");
  });

  it("renders a dash when there is no token data at all", () => {
    const totals: TokenTotals = { input: 0, output: 0, total: 0 };
    expect(formatTotalTokens(totals)).toBe("-");
  });

  it("renders a dash when the old backend omits the field", () => {
    expect(formatTotalTokens(undefined)).toBe("-");
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
