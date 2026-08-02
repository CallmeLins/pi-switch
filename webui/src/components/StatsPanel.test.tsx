import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StatsPanel } from "./StatsPanel";
import type { UsageStats } from "../types";

const statsMock = vi.fn();

vi.mock("../api", () => ({
  api: { stats: () => statsMock() },
  logsExportUrl: (format: "json" | "csv") => `/api/logs/export?format=${format}`,
}));

function fullStats(): UsageStats {
  return {
    totalRequests: 10,
    okRequests: 9,
    failedRequests: 1,
    successRate: "90.0%",
    avgLatencyMs: 42,
    byProvider: {
      hyb: {
        total: 6,
        ok: 5,
        failed: 1,
        retries: 0,
        avgMs: 40,
        totalMs: 240,
        lastUsed: "2026-08-02T10:00:00Z",
        promptTokens: 300_000,
        outputTokens: 50_000,
        cachedTokens: 200_000,
      },
      fox: {
        total: 4,
        ok: 4,
        failed: 0,
        retries: 0,
        avgMs: 45,
        totalMs: 180,
        lastUsed: undefined,
        promptTokens: 12_300,
        outputTokens: 1_200,
        cachedTokens: 0,
      },
    },
    totalTokens: { input: 312_300, output: 51_200, total: 363_500 },
    cacheHitRate: "53.3%",
    byConversation: [
      {
        conversationId: "unlabeled",
        requests: 3,
        inputTokens: 0,
        outputTokens: 0,
        lastActive: "2026-08-02T10:05:00Z",
      },
      {
        conversationId: "conv-a1b2c3d4e5f6g7",
        requests: 5,
        inputTokens: 300_000,
        outputTokens: 50_000,
        lastActive: "2026-08-02T10:03:00Z",
      },
      {
        conversationId: "conv-x",
        requests: 2,
        inputTokens: 12_300,
        outputTokens: 1_200,
        lastActive: "2026-08-02T09:00:00Z",
      },
    ],
  };
}

function legacyStats(): UsageStats {
  return {
    totalRequests: 4,
    okRequests: 3,
    failedRequests: 1,
    successRate: "75.0%",
    avgLatencyMs: 30,
    byProvider: {
      hyb: {
        total: 4,
        ok: 3,
        failed: 1,
        retries: 0,
        avgMs: 30,
        totalMs: 120,
        lastUsed: "2026-07-01T00:00:00Z",
        promptTokens: 0,
        outputTokens: 0,
        cachedTokens: 0,
      },
    },
    totalTokens: { input: 0, output: 0, total: 0 },
    cacheHitRate: "-",
    byConversation: [],
  };
}

describe("StatsPanel", () => {
  beforeEach(() => {
    statsMock.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the empty state when there is no request data", async () => {
    statsMock.mockResolvedValue({
      totalRequests: 0,
      okRequests: 0,
      failedRequests: 0,
      successRate: "0%",
      byProvider: {},
      totalTokens: { input: 0, output: 0, total: 0 },
      cacheHitRate: "-",
      byConversation: [],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    expect(await screen.findByText(/No request data yet/)).toBeInTheDocument();
    expect(screen.queryByText(/By conversation/)).not.toBeInTheDocument();
  });

  it("renders token cards, provider token column and conversation list with data", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("363.5K")).toBeInTheDocument();
    expect(screen.getAllByText("Tokens").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("Cache 率")).toBeInTheDocument();
    expect(screen.getByText("53.3%")).toBeInTheDocument();

    expect(screen.getByText("hyb")).toBeInTheDocument();
    expect(screen.getAllByText("350.0K").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("13.5K").length).toBeGreaterThanOrEqual(2);

    expect(screen.getByText("By conversation")).toBeInTheDocument();
    expect(screen.getByText("conv-a1b2c3d…")).toBeInTheDocument();
    expect(screen.getByText("unlabeled")).toBeInTheDocument();
    expect(screen.getByText("3 requests")).toBeInTheDocument();
  });

  it("renders dashes for token metrics when only legacy data exists", async () => {
    statsMock.mockResolvedValue(legacyStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect((await screen.findAllByText("Tokens")).length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("-").length).toBeGreaterThanOrEqual(3);
    expect(screen.getAllByText("4").length).toBeGreaterThanOrEqual(1);
    expect(screen.queryByText(/By conversation/)).not.toBeInTheDocument();
  });

  it("tolerates an old backend that omits the token fields", async () => {
    statsMock.mockResolvedValue({
      totalRequests: 2,
      okRequests: 2,
      failedRequests: 0,
      successRate: "100.0%",
      avgLatencyMs: 20,
      byProvider: {
        hyb: {
          total: 2,
          ok: 2,
          failed: 0,
          retries: 0,
          avgMs: 20,
          totalMs: 40,
          lastUsed: undefined,
          promptTokens: 0,
          outputTokens: 0,
          cachedTokens: 0,
        },
      },
    } as never);
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect((await screen.findAllByText("2")).length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("-").length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByText(/By conversation/)).not.toBeInTheDocument();
  });

  it("keeps the existing request metrics and export actions", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("10")).toBeInTheDocument();
    expect(screen.getByText("90.0%")).toBeInTheDocument();
    expect(screen.getByText("Export JSON")).toBeInTheDocument();
    expect(screen.getByText("Export CSV")).toBeInTheDocument();
    expect(screen.getByText("Refresh")).toBeInTheDocument();
  });
});
