import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StatsPanel } from "./StatsPanel";
import type { UsageStats } from "../types";

const statsMock = vi.fn();

vi.mock("../api", () => ({
  api: { stats: (...args: unknown[]) => statsMock(...args) },
  logsExportUrl: (format: "json" | "csv") => `/api/logs/export?format=${format}`,
}));

const FIXED_NOW = new Date(2026, 7, 2, 15, 30, 0).getTime();

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
        reasoningTokens: 20_000,
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
        reasoningTokens: 0,
      },
    },
    totalTokens: {
      input: 312_300,
      output: 51_200,
      total: 363_500,
      cached: 200_000,
      reasoning: 20_000,
    },
    cacheHitRate: "53.3%",
    byConversation: [
      {
        conversationId: "unlabeled",
        requests: 3,
        inputTokens: 0,
        outputTokens: 0,
        cachedTokens: 0,
        reasoningTokens: 0,
        lastActive: "2026-08-02T10:05:00Z",
      },
      {
        conversationId: "conv-a1b2c3d4e5f6g7",
        requests: 5,
        inputTokens: 300_000,
        outputTokens: 50_000,
        cachedTokens: 200_000,
        reasoningTokens: 20_000,
        lastActive: "2026-08-02T10:03:00Z",
      },
      {
        conversationId: "conv-x",
        requests: 2,
        inputTokens: 12_300,
        outputTokens: 1_200,
        cachedTokens: 0,
        reasoningTokens: 0,
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
        reasoningTokens: 0,
      },
    },
    totalTokens: { input: 0, output: 0, total: 0, cached: 0, reasoning: 0 },
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
    vi.useRealTimers();
    vi.restoreAllMocks();
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
    expect(screen.getAllByText("Tokens").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Cache 率")).toBeInTheDocument();
    expect(screen.getByText("53.3%")).toBeInTheDocument();

    expect(screen.getByText("hyb")).toBeInTheDocument();
    expect(screen.getAllByText("350.0K").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("13.5K").length).toBeGreaterThanOrEqual(1);

    expect(screen.getByText("By conversation")).toBeInTheDocument();
    expect(screen.getByText("conv-a1b2c3d…")).toBeInTheDocument();
    expect(screen.getByText("unlabeled")).toBeInTheDocument();
    expect(screen.getByText("3 requests")).toBeInTheDocument();
  });

  it("renders dashes for token metrics when only legacy data exists", async () => {
    statsMock.mockResolvedValue(legacyStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect((await screen.findAllByText("Tokens")).length).toBeGreaterThanOrEqual(1);
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

  it("renders five token cards with subset badges", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("312.3K")).toBeInTheDocument();
    expect(screen.getByText("51.2K")).toBeInTheDocument();
    expect(screen.getByText("200.0K")).toBeInTheDocument();
    expect(screen.getAllByText("20.0K").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("363.5K")).toBeInTheDocument();
    expect(screen.getByText("⊆ Input")).toBeInTheDocument();
    expect(screen.getByText("⊆ Output")).toBeInTheDocument();
  });

  it("shows cache, reasoning and total columns per conversation", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("Cached 200.0K")).toBeInTheDocument();
    expect(screen.getByText("Reasoning 20.0K")).toBeInTheDocument();
    expect(screen.getByText("Total 350.0K")).toBeInTheDocument();
    expect(screen.getByText("Total 13.5K")).toBeInTheDocument();
    expect(screen.getByText("Total -")).toBeInTheDocument();
  });

  it("keeps rendering legacy data that lacks cache/reasoning fields", async () => {
    statsMock.mockResolvedValue({
      totalRequests: 2,
      okRequests: 2,
      failedRequests: 0,
      successRate: "100.0%",
      byProvider: {
        hyb: {
          total: 2,
          ok: 2,
          failed: 0,
          retries: 0,
          avgMs: 20,
          totalMs: 40,
          promptTokens: 1_000,
          outputTokens: 500,
          cachedTokens: 0,
        },
      },
      totalTokens: { input: 1_000, output: 500, total: 1_500 },
      cacheHitRate: "0%",
      byConversation: [
        { conversationId: "conv-old", requests: 1, inputTokens: 1_000, outputTokens: 500 },
      ],
    } as never);
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect((await screen.findAllByText("1.5K")).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Cached -")).toBeInTheDocument();
    expect(screen.getByText("Reasoning -")).toBeInTheDocument();
    expect(screen.getByText("Total 1.5K")).toBeInTheDocument();
    expect(screen.getByText("conv-old")).toBeInTheDocument();
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

  it("renders the request details table with full token columns", async () => {
    statsMock.mockResolvedValue({
      ...fullStats(),
      recentRequests: [
        {
          ts: "2026-08-02T10:00:00Z",
          provider: "hyb",
          model: "deepseek-chat",
          ok: true,
          status: 200,
          error: null,
          promptTokens: 1234,
          completionTokens: 567,
          cachedTokens: 890,
          reasoningTokens: 100,
          totalTokens: 1801,
          cacheRate: "72.1%",
        },
      ],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("Request details")).toBeInTheDocument();
    expect(screen.getByText("deepseek-chat")).toBeInTheDocument();
    expect(screen.getByText("1.2K")).toBeInTheDocument();
    expect(screen.getByText("567")).toBeInTheDocument();
    expect(screen.getByText("890")).toBeInTheDocument();
    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("1.8K")).toBeInTheDocument();
    expect(screen.getByText("72.1%")).toBeInTheDocument();
    expect(screen.getByText("200")).toBeInTheDocument();
    const expectedTime = new Date("2026-08-02T10:00:00Z").toLocaleTimeString("en-GB", {
      hour12: false,
    });
    expect(screen.getByText(expectedTime)).toBeInTheDocument();
  });

  it("renders dashes for rows without usage and shows status plus error for failures", async () => {
    statsMock.mockResolvedValue({
      ...fullStats(),
      recentRequests: [
        {
          ts: "2026-08-02T10:00:00Z",
          provider: "hyb",
          model: "deepseek-chat",
          ok: true,
          status: 200,
          error: null,
          promptTokens: 1234,
          completionTokens: 567,
          cachedTokens: 890,
          reasoningTokens: 100,
          totalTokens: 1801,
          cacheRate: "72.1%",
        },
        {
          ts: "2026-08-02T10:01:00Z",
          provider: "hyb",
          model: "deepseek-chat",
          ok: false,
          status: 429,
          error: "rate limited by provider",
          promptTokens: null,
          completionTokens: null,
          cachedTokens: null,
          reasoningTokens: null,
          totalTokens: null,
          cacheRate: "-",
        },
        {
          ts: "2026-08-02T10:02:00Z",
          provider: "fox",
          model: "gpt-4o",
          ok: true,
          status: 200,
          error: null,
          promptTokens: null,
          completionTokens: null,
          cachedTokens: null,
          reasoningTokens: null,
          totalTokens: null,
          cacheRate: "-",
        },
      ],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("Request details")).toBeInTheDocument();
    expect(screen.getByText("429 rate limited by provider")).toBeInTheDocument();
    expect(screen.getByText("gpt-4o")).toBeInTheDocument();

    const rows = within(screen.getByRole("table", { name: "Request details" })).getAllByRole("row");
    expect(within(rows[1]).getByText("200")).toBeInTheDocument();
    expect(within(rows[1]).getByText("72.1%")).toBeInTheDocument();
    expect(within(rows[2]).getAllByText("-").length).toBe(6);
    expect(within(rows[3]).getAllByText("-").length).toBe(6);
  });

  it("shows input, output and cache rate per conversation alongside the existing dimensions", async () => {
    statsMock.mockResolvedValue({
      ...fullStats(),
      byConversation: [
        {
          conversationId: "conv-a1b2c3d4e5f6g7h8i9",
          requests: 3,
          inputTokens: 312300,
          outputTokens: 51200,
          cachedTokens: 200000,
          reasoningTokens: 20000,
          lastActive: "2026-08-02T10:00:00Z",
          cacheRate: "64.1%",
        },
        {
          conversationId: "unlabeled",
          requests: 2,
          inputTokens: 0,
          outputTokens: 0,
          cachedTokens: 0,
          reasoningTokens: 0,
          lastActive: null,
          cacheRate: "-",
        },
        {
          conversationId: "conv-z9y8x7w6v5u4t3s2r1q0",
          requests: 5,
          inputTokens: 13500,
          outputTokens: 0,
          cachedTokens: 0,
          reasoningTokens: 0,
          lastActive: "2026-08-02T10:01:00Z",
          cacheRate: "0.0%",
        },
      ],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("Input 312.3K")).toBeInTheDocument();
    expect(screen.getByText("Output 51.2K")).toBeInTheDocument();
    expect(screen.getByText("Rate 64.1%")).toBeInTheDocument();
    expect(screen.getByText("Input 13.5K")).toBeInTheDocument();
    expect(screen.getByText("Rate 0.0%")).toBeInTheDocument();
    expect(screen.getAllByText("Input -").length).toBe(1);
    expect(screen.getAllByText("Output -").length).toBe(2);
    expect(screen.getAllByText("Rate -").length).toBe(1);

    expect(screen.getByText("Cached 200.0K")).toBeInTheDocument();
    expect(screen.getByText("Reasoning 20.0K")).toBeInTheDocument();
    expect(screen.getByText("Total 363.5K")).toBeInTheDocument();
    expect(screen.getByText("Total 13.5K")).toBeInTheDocument();
  });

  it("does not render the request details card when recentRequests is empty or absent", async () => {
    statsMock.mockResolvedValue({ ...fullStats(), recentRequests: [] });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");
    expect(screen.queryByText("Request details")).not.toBeInTheDocument();

    cleanup();
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");
    expect(screen.queryByText("Request details")).not.toBeInTheDocument();
  });

  it("renders the four window presets with today selected by default", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("363.5K")).toBeInTheDocument();
    const today = screen.getByRole("button", { name: "Today" });
    expect(today).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "24h" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "7d" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Custom" })).toBeInTheDocument();
    expect(today).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "24h" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("reveals the custom date inputs only in custom mode", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    await screen.findByText("363.5K");
    expect(screen.queryByLabelText("From")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    expect(screen.getByLabelText("From")).toBeInTheDocument();
    expect(screen.getByLabelText("To")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Custom" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    fireEvent.click(screen.getByRole("button", { name: "Today" }));
    expect(screen.queryByLabelText("From")).not.toBeInTheDocument();
  });

  it("sends local window bounds with the initial load and each preset switch", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    await screen.findByText("363.5K");
    expect(statsMock).toHaveBeenLastCalledWith(
      "today",
      new Date(2026, 7, 2, 0, 0, 0, 0).getTime(),
      FIXED_NOW,
    );

    fireEvent.click(screen.getByRole("button", { name: "24h" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "last24h",
        FIXED_NOW - 24 * 3600 * 1000,
        FIXED_NOW,
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "7d" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "last7d",
        FIXED_NOW - 7 * 24 * 3600 * 1000,
        FIXED_NOW,
      ),
    );
  });

  it("custom defaults to today and re-requests when a date changes", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 2, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 3, 0, 0, 0, 0).getTime(),
      ),
    );

    fireEvent.change(screen.getByLabelText("From"), {
      target: { value: "2026-08-01" },
    });
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 1, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 3, 0, 0, 0, 0).getTime(),
      ),
    );

    fireEvent.change(screen.getByLabelText("To"), {
      target: { value: "2026-08-04" },
    });
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 1, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 5, 0, 0, 0, 0).getTime(),
      ),
    );
  });

  it("re-renders with the data of the selected window", async () => {
    statsMock.mockResolvedValueOnce(fullStats());
    statsMock.mockResolvedValueOnce({ ...fullStats(), totalRequests: 3 });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: "24h" }));
    expect(await screen.findByText("3")).toBeInTheDocument();
  });

  it("does not request and shows a hint when the custom end precedes the start", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    await waitFor(() => expect(statsMock).toHaveBeenCalledTimes(2));
    statsMock.mockClear();

    fireEvent.change(screen.getByLabelText("From"), {
      target: { value: "2026-08-05" },
    });
    fireEvent.change(screen.getByLabelText("To"), {
      target: { value: "2026-08-02" },
    });
    expect(statsMock).not.toHaveBeenCalled();
    expect(screen.getByText("End must be on or after start")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("To"), {
      target: { value: "2026-08-06" },
    });
    await waitFor(() => expect(statsMock).toHaveBeenCalledTimes(1));
    expect(statsMock).toHaveBeenLastCalledWith(
      "custom",
      new Date(2026, 7, 5, 0, 0, 0, 0).getTime(),
      new Date(2026, 7, 7, 0, 0, 0, 0).getTime(),
    );
    expect(screen.queryByText("End must be on or after start")).not.toBeInTheDocument();
  });

  it("does not request a stale invalid custom window when re-entering custom mode", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    await waitFor(() => expect(statsMock).toHaveBeenCalledTimes(2));
    statsMock.mockClear();

    fireEvent.change(screen.getByLabelText("From"), {
      target: { value: "2026-08-05" },
    });
    fireEvent.change(screen.getByLabelText("To"), {
      target: { value: "2026-08-02" },
    });
    await waitFor(() =>
      expect(screen.getByText("End must be on or after start")).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Today" }));
    statsMock.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Custom" }));

    expect(screen.getByText("End must be on or after start")).toBeInTheDocument();
    expect(statsMock).not.toHaveBeenCalled();
  });

  it("prompts for both dates and skips the request when a custom date is cleared", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    await waitFor(() => expect(statsMock).toHaveBeenCalledTimes(2));
    statsMock.mockClear();

    fireEvent.change(screen.getByLabelText("From"), { target: { value: "" } });
    expect(screen.getByText("Select both start and end dates")).toBeInTheDocument();
    expect(statsMock).not.toHaveBeenCalled();
  });
});
