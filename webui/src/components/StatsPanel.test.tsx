import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StatsPanel } from "./StatsPanel";
import type { ConversationRequestsPage, ConversationStats, ConversationsPage, RecentRequest, UsageStats } from "../types";

const statsMock = vi.fn();
const convMock = vi.fn();
const convReqMock = vi.fn();

vi.mock("../api", () => ({
  api: {
    stats: (...args: unknown[]) => statsMock(...args),
    statsConversations: (...args: unknown[]) => convMock(...args),
    conversationRequests: (...args: unknown[]) => convReqMock(...args),
  },
  logsExportUrl: (format: "json" | "csv") => `/api/logs/export?format=${format}`,
}));

const FIXED_NOW = new Date(2026, 7, 2, 15, 30, 0).getTime();

/** Local "YYYY-MM-DD HH:MM:SS" rendering, mirroring formatRequestTime. */
function fullTime(ts: string): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(
    d.getMinutes(),
  )}:${p(d.getSeconds())}`;
}

function convPage(conversations: ConversationStats[] = []): ConversationsPage {
  return { conversations, total: conversations.length };
}

function reqPage(requests: RecentRequest[] = []): ConversationRequestsPage {
  return { requests, total: requests.length };
}


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
  };
}

describe("StatsPanel", () => {
  beforeEach(() => {
    statsMock.mockReset();
    convMock.mockReset();
    convMock.mockResolvedValue(convPage());
    convReqMock.mockReset();
    convReqMock.mockResolvedValue(reqPage());
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
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    expect(await screen.findByText(/No request data yet/)).toBeInTheDocument();
    expect(screen.queryByText(/By conversation/)).not.toBeInTheDocument();
  });

  it("mounts and fetches both windows in parallel", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    expect(statsMock).toHaveBeenCalledTimes(1);
    expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 0, 50);
    expect(convMock).toHaveBeenCalledTimes(1);
    expect(convMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 0, 50);
  });

  it("renders the total cost card with an unknown hint", async () => {
    statsMock.mockResolvedValue({ ...fullStats(), totalCost: 12.34, costUnknown: 2 });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("$12.34")).toBeInTheDocument();
    expect(screen.getByText("Cost")).toBeInTheDocument();
    expect(screen.getByText(/2 unknown/)).toBeInTheDocument();
  });

  it("renders a dash for the cost card when the total cost is unknown", async () => {
    statsMock.mockResolvedValue({ ...fullStats(), totalCost: null, costUnknown: 10 });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("Cost")).toBeInTheDocument();
    expect(screen.queryByText("$0.00")).not.toBeInTheDocument();
    expect(screen.getByText(/10 unknown/)).toBeInTheDocument();
  });

  it("keeps the cost card off the empty state", async () => {
    statsMock.mockResolvedValue({
      totalRequests: 0,
      okRequests: 0,
      failedRequests: 0,
      successRate: "0%",
      byProvider: {},
      totalTokens: { input: 0, output: 0, total: 0 },
      cacheHitRate: "-",
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    expect(await screen.findByText(/No request data yet/)).toBeInTheDocument();
    expect(screen.queryByText("Cost")).not.toBeInTheDocument();
  });

  it("renders token cards, provider token column and the conversation table", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(
      convPage([
        { conversationId: "conv-a1b2c3d4e5f6g7", requests: 5, inputTokens: 300_000, outputTokens: 50_000, cachedTokens: 200_000, reasoningTokens: 20_000, lastActive: "2026-08-02T10:03:00Z" },
        { conversationId: "unlabeled", requests: 3, inputTokens: 0, outputTokens: 0, cachedTokens: 0, reasoningTokens: 0, lastActive: "2026-08-02T10:05:00Z" },
      ]),
    );
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("363.5K")).toBeInTheDocument();
    expect(screen.getAllByText("Tokens").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Cache rate")).toBeInTheDocument();
    expect(screen.getByText("53.3%")).toBeInTheDocument();

    expect(screen.getByText("hyb")).toBeInTheDocument();
    expect(screen.getAllByText("350.0K").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("13.5K").length).toBeGreaterThanOrEqual(1);

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const table = await screen.findByRole("table", { name: "By conversation" });
    expect(within(table).getByText("conv-a1b2c3d…")).toBeInTheDocument();
    expect(within(table).getByText("unlabeled")).toBeInTheDocument();
    expect(within(table).getByText("5")).toBeInTheDocument();
    expect(within(table).getByText("3")).toBeInTheDocument();
  });

  it("renders dashes for token metrics when only legacy data exists", async () => {
    statsMock.mockResolvedValue(legacyStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect((await screen.findAllByText("Tokens")).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("-").length).toBeGreaterThanOrEqual(3);
    expect(screen.getAllByText("4").length).toBeGreaterThanOrEqual(1);

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    expect(await screen.findByText("No conversation data in this range.")).toBeInTheDocument();
  });

  it("shows the cost per conversation and per request", async () => {
    statsMock.mockResolvedValue({
      ...fullStats(),
      totalCost: 0.75,
      recentRequests: [
        {
          ts: "2026-08-02T10:00:00Z",
          provider: "hyb",
          model: "m1",
          ok: true,
          status: 200,
          error: null,
          promptTokens: 100,
          completionTokens: 10,
          cachedTokens: 0,
          reasoningTokens: 0,
          totalTokens: 110,
          cacheRate: "0.0%",
          cost: 0.75,
        },
        {
          ts: "2026-08-02T10:01:00Z",
          provider: "hyb",
          model: "m2",
          ok: true,
          status: 200,
          error: null,
          promptTokens: null,
          completionTokens: null,
          cachedTokens: null,
          reasoningTokens: null,
          totalTokens: null,
          cacheRate: "-",
          cost: null,
        },
      ],
    });
    convMock.mockResolvedValue(
      convPage([
        {
          conversationId: "conv-a",
          requests: 2,
          inputTokens: 100,
          outputTokens: 10,
          cachedTokens: 0,
          reasoningTokens: 0,
          lastActive: "2026-08-02T10:00:00Z",
          cacheRate: "0.0%",
          cost: 0.75,
        },
        {
          conversationId: "unlabeled",
          requests: 1,
          inputTokens: 100,
          outputTokens: 10,
          cachedTokens: 0,
          reasoningTokens: 0,
          cacheRate: "-",
          cost: null,
        },
      ]),
    );
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    await screen.findByText("363.5K");
    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const convTable = await screen.findByRole("table", { name: "By conversation" });
    const convRows = within(convTable).getAllByRole("row");
    expect(within(convRows[1]).getByText("$0.75")).toBeInTheDocument();
    expect(within(convRows[2]).getAllByText("-").length).toBeGreaterThanOrEqual(3);

    const rows = within(screen.getByRole("table", { name: "Request details" })).getAllByRole("row");
    expect(within(rows[1]).getByText("$0.75")).toBeInTheDocument();
    // 7 token columns + Session column render "-" for the bare row.
    expect(within(rows[2]).getAllByText("-").length).toBe(8);
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
    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    expect(await screen.findByText("No conversation data in this range.")).toBeInTheDocument();
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
    convMock.mockResolvedValue(
      convPage([
        { conversationId: "conv-a", requests: 2, inputTokens: 300_000, outputTokens: 50_000, cachedTokens: 200_000, reasoningTokens: 20_000, lastActive: "2026-08-02T10:00:00Z" },
        { conversationId: "conv-x", requests: 1, inputTokens: 12_300, outputTokens: 1_200, cachedTokens: 0, reasoningTokens: 0, lastActive: "2026-08-02T09:00:00Z" },
        { conversationId: "unlabeled", requests: 1, inputTokens: 0, outputTokens: 0, cachedTokens: 0, reasoningTokens: 0 },
      ]),
    );
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");
    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const table = await screen.findByRole("table", { name: "By conversation" });

    expect(within(table).getByText("200.0K")).toBeInTheDocument();
    expect(within(table).getByText("20.0K")).toBeInTheDocument();
    expect(within(table).getByText("350.0K")).toBeInTheDocument();
    expect(within(table).getByText("13.5K")).toBeInTheDocument();
    expect(within(table).getAllByText("0").length).toBeGreaterThanOrEqual(4);
    // The cache-rate column header must be labelled "Cache rate", not the
    // success-rate label "Rate" used by the by-provider table.
    expect(within(table).getByText("Cache rate")).toBeInTheDocument();
    expect(within(table).queryByText("Rate")).not.toBeInTheDocument();
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
    } as never);
    convMock.mockResolvedValue(
      convPage([
        { conversationId: "conv-old", requests: 1, inputTokens: 1_000, outputTokens: 500, cachedTokens: 0, reasoningTokens: 0 },
      ]),
    );
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect((await screen.findAllByText("1.5K")).length).toBeGreaterThanOrEqual(1);
    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const table = await screen.findByRole("table", { name: "By conversation" });
    expect(within(table).getByText("conv-old")).toBeInTheDocument();
    expect(within(table).getByText("1.0K")).toBeInTheDocument();
    expect(within(table).getByText("500")).toBeInTheDocument();
    expect(within(table).getAllByText("-").length).toBeGreaterThanOrEqual(3);
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

  it("renders the request details table with full token columns and full time", async () => {
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
    const reqTable = screen.getByRole("table", { name: "Request details" });
    // The cache-rate column header must be labelled "Cache rate", not the
    // success-rate label "Rate" used by the by-provider table.
    expect(within(reqTable).getByText("Cache rate")).toBeInTheDocument();
    expect(within(reqTable).queryByText("Rate")).not.toBeInTheDocument();
    expect(screen.getByText("deepseek-chat")).toBeInTheDocument();
    expect(screen.getByText("1.2K")).toBeInTheDocument();
    expect(screen.getByText("567")).toBeInTheDocument();
    expect(screen.getByText("890")).toBeInTheDocument();
    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("1.8K")).toBeInTheDocument();
    expect(screen.getByText("72.1%")).toBeInTheDocument();
    expect(screen.getByText("200")).toBeInTheDocument();
    expect(screen.getByText(fullTime("2026-08-02T10:00:00Z"))).toBeInTheDocument();
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
    // 7 token columns + Session column render "-" for rows without an id.
    expect(within(rows[2]).getAllByText("-").length).toBe(8);
    expect(within(rows[3]).getAllByText("-").length).toBe(8);
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

  it("renders request details above the conversation list", async () => {
    statsMock.mockResolvedValue({
      ...fullStats(),
      recentRequests: [
        {
          ts: "2026-08-02T10:00:00Z",
          provider: "hyb",
          model: "m1",
          ok: true,
          status: 200,
          error: null,
        },
      ],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("Request details");

    const details = screen.getByText("Request details");
    const conv = screen.getByText("By conversation");
    expect(details.compareDocumentPosition(conv) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it("collapses request details by default and hides the table when collapsed", async () => {
    statsMock.mockResolvedValue({
      ...fullStats(),
      recentRequests: [
        {
          ts: "2026-08-02T10:00:00Z",
          provider: "hyb",
          model: "m1",
          ok: true,
          status: 200,
          error: null,
        },
      ],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    const header = await screen.findByRole("button", { name: /Request details/ });
    expect(header).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("table", { name: "Request details" })).toBeInTheDocument();

    fireEvent.click(header);
    expect(header).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("table", { name: "Request details" })).not.toBeInTheDocument();

    fireEvent.click(header);
    expect(header).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("table", { name: "Request details" })).toBeInTheDocument();
  });

  it("collapses conversations by default and toggles on header click", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(convPage([{ conversationId: "unlabeled", requests: 3, inputTokens: 0, outputTokens: 0, cachedTokens: 0, reasoningTokens: 0 }]));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    expect(screen.queryByRole("table", { name: "By conversation" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /By conversation/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    expect(await screen.findByRole("table", { name: "By conversation" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /By conversation/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    expect(screen.queryByRole("table", { name: "By conversation" })).not.toBeInTheDocument();
  });

  it("shows the conversation name and falls back to the truncated id", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(
      convPage([
        {
          conversationId: "conv-a1b2c3d4e5f6g7h8i9",
          name: "My chat",
          requests: 3,
          inputTokens: 100,
          outputTokens: 10,
          cachedTokens: 0,
          reasoningTokens: 0,
          lastActive: "2026-08-02T10:00:00Z",
        },
        {
          conversationId: "conv-z9y8x7w6v5u4t3s2r1q0",
          requests: 5,
          inputTokens: 100,
          outputTokens: 10,
          cachedTokens: 0,
          reasoningTokens: 0,
          lastActive: "2026-08-02T10:01:00Z",
        },
      ]),
    );
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");
    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const table = await screen.findByRole("table", { name: "By conversation" });

    expect(within(table).getByText("My chat")).toBeInTheDocument();
    expect(within(table).queryByText("conv-a1b2c3d…")).not.toBeInTheDocument();
    const unnamed = within(table).getByText("conv-z9y8x7w…");
    expect(unnamed).toHaveAttribute("title", "conv-z9y8x7w6v5u4t3s2r1q0");
    expect(within(table).getByText(fullTime("2026-08-02T10:01:00Z"))).toBeInTheDocument();
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
    expect(screen.getByRole("button", { name: "24h" })).toHaveAttribute("aria-pressed", "false");
  });

  it("reveals the custom date inputs only in custom mode", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    await screen.findByText("363.5K");
    expect(screen.queryByLabelText("From")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    expect(screen.getByLabelText("From")).toBeInTheDocument();
    expect(screen.getByLabelText("To")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Custom" })).toHaveAttribute("aria-pressed", "true");

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
      0,
      50,
    );

    fireEvent.click(screen.getByRole("button", { name: "24h" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "last24h",
        FIXED_NOW - 24 * 3600 * 1000,
        FIXED_NOW,
        0,
        50,
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "7d" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "last7d",
        FIXED_NOW - 7 * 24 * 3600 * 1000,
        FIXED_NOW,
        0,
        50,
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
        0,
        50,
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
        0,
        50,
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
        0,
        50,
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
      0,
      50,
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
    fireEvent.change(screen.getByLabelText("From"), { target: { value: "" } });
    expect(screen.getByText("Select both start and end dates")).toBeInTheDocument();
    expect(statsMock).not.toHaveBeenCalled();
  });

  it("offers the four auto-refresh tiers with Off selected by default", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    const select = screen.getByLabelText(/Auto-refresh/) as HTMLSelectElement;
    expect(select.value).toBe("off");
    const labels = Array.from(select.querySelectorAll("option")).map((o) => o.textContent);
    expect(labels).toEqual(["Off", "5s", "30s", "5min"]);
  });

  it("auto-refreshes on the selected interval reusing the current window", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    vi.useFakeTimers();
    fireEvent.change(screen.getByLabelText(/Auto-refresh/), { target: { value: "5000" } });
    statsMock.mockClear();

    await vi.advanceTimersByTimeAsync(5000);
    expect(statsMock).toHaveBeenLastCalledWith(
      "today",
      new Date(2026, 7, 2, 0, 0, 0, 0).getTime(),
      expect.any(Number),
      0,
      50,
    );

    await vi.advanceTimersByTimeAsync(5000);
    expect(statsMock).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });

  it("auto-refresh also refreshes the conversation window with its own bounds", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    vi.useFakeTimers();
    fireEvent.change(screen.getByLabelText(/Auto-refresh/), { target: { value: "5000" } });
    statsMock.mockClear();
    convMock.mockClear();

    await vi.advanceTimersByTimeAsync(5000);
    expect(statsMock).toHaveBeenCalledTimes(1);
    expect(convMock).toHaveBeenCalledTimes(1);
    expect(convMock).toHaveBeenLastCalledWith(
      "today",
      new Date(2026, 7, 2, 0, 0, 0, 0).getTime(),
      expect.any(Number),
      0,
      50,
    );
    vi.useRealTimers();
  });

  it("stops polling when switched back to Off", async () => {
    statsMock.mockResolvedValue(fullStats());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    vi.useFakeTimers();
    const select = screen.getByLabelText(/Auto-refresh/);
    fireEvent.change(select, { target: { value: "5000" } });
    await vi.advanceTimersByTimeAsync(5000);
    statsMock.mockClear();

    fireEvent.change(select, { target: { value: "off" } });
    await vi.advanceTimersByTimeAsync(15_000);
    expect(statsMock).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("keeps the current data when an auto-refresh fails", async () => {
    statsMock.mockResolvedValueOnce(fullStats());
    statsMock.mockRejectedValueOnce(new Error("boom"));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    vi.useFakeTimers();
    fireEvent.change(screen.getByLabelText(/Auto-refresh/), { target: { value: "5000" } });
    await vi.advanceTimersByTimeAsync(5000);

    expect(screen.getByText("363.5K")).toBeInTheDocument();
    expect(screen.queryByText(/No request data yet/)).not.toBeInTheDocument();
    vi.useRealTimers();
  });

  it("clears the timer on unmount", async () => {
    statsMock.mockResolvedValue(fullStats());
    const { unmount } = render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    vi.useFakeTimers();
    fireEvent.change(screen.getByLabelText(/Auto-refresh/), { target: { value: "5000" } });
    statsMock.mockClear();

    unmount();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(statsMock).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("renders pagination controls with total count, page buttons and boundary disabling", async () => {
    const rows = Array.from({ length: 50 }, (_, i) => ({
      ts: `2026-08-02T10:00:${String(i).padStart(2, "0")}Z`,
      provider: "hyb",
      model: "m1",
      ok: true,
      status: 200,
      error: null,
    }));
    statsMock.mockResolvedValue({ ...fullStats(), recentRequestTotal: 250, recentRequests: rows });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("Request details")).toBeInTheDocument();
    expect(screen.getByText("250 rows")).toBeInTheDocument();
    for (const n of ["1", "2", "3", "4", "5"]) {
      expect(screen.getByRole("button", { name: n })).toBeInTheDocument();
    }
    expect(screen.getByRole("button", { name: "1" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Previous page" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next page" })).toBeEnabled();
  });

  it("requests the selected page via next and page buttons, keeping aggregate cards", async () => {
    const makeRows = (model: string) =>
      Array.from({ length: 50 }, (_, i) => ({
        ts: `2026-08-02T10:00:${String(i).padStart(2, "0")}Z`,
        provider: "hyb",
        model: `${model}-${i}`,
        ok: true,
        status: 200,
        error: null,
      }));
    statsMock
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows("m-p0") })
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows("m-p1") })
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows("m-p4") });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("m-p0-0")).toBeInTheDocument();
    expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 0, 50);

    fireEvent.click(screen.getByRole("button", { name: "Next page" }));
    expect(await screen.findByText("m-p1-0")).toBeInTheDocument();
    expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 1, 50);

    fireEvent.click(screen.getByRole("button", { name: "5" }));
    expect(await screen.findByText("m-p4-0")).toBeInTheDocument();
    expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 4, 50);

    expect(screen.getByText("363.5K")).toBeInTheDocument();
  });

  it("switches rows per page, resetting to page 1 and re-requesting with the new limit", async () => {
    const makeRows = (n: number, tag: string) =>
      Array.from({ length: n }, (_, i) => ({
        ts: `2026-08-02T10:00:${String(i).padStart(2, "0")}Z`,
        provider: "hyb",
        model: `${tag}-${i}`,
        ok: true,
        status: 200,
        error: null,
      }));
    statsMock
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows(50, "a") })
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows(50, "b") })
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows(100, "c") });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("a-0")).toBeInTheDocument();
    expect(screen.getByLabelText("Rows per page")).toHaveValue("50");

    fireEvent.click(screen.getByRole("button", { name: "3" }));
    expect(await screen.findByText("b-0")).toBeInTheDocument();
    expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 2, 50);

    fireEvent.change(screen.getByLabelText("Rows per page"), { target: { value: "100" } });
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 0, 100),
    );
    expect(await screen.findByText("c-0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "1" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "3" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "4" })).not.toBeInTheDocument();
  });

  it("resets to page 1 when switching stats windows (preset and custom date change)", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const makeRows = (tag: string) =>
      Array.from({ length: 50 }, (_, i) => ({
        ts: `2026-08-02T10:00:${String(i).padStart(2, "0")}Z`,
        provider: "hyb",
        model: `${tag}-${i}`,
        ok: true,
        status: 200,
        error: null,
      }));
    const paged = (tag: string) => ({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows(tag) });
    statsMock
      .mockResolvedValueOnce(paged("a"))
      .mockResolvedValueOnce(paged("b"))
      .mockResolvedValueOnce(paged("c"))
      .mockResolvedValueOnce(paged("d"))
      .mockResolvedValueOnce(paged("e"));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("a-0")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "3" }));
    expect(await screen.findByText("b-0")).toBeInTheDocument();
    expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 2, 50);

    fireEvent.click(screen.getByRole("button", { name: "24h" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "last24h",
        FIXED_NOW - 24 * 3600 * 1000,
        FIXED_NOW,
        0,
        50,
      ),
    );
    expect(await screen.findByText("c-0")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 2, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 3, 0, 0, 0, 0).getTime(),
        0,
        50,
      ),
    );
    expect(await screen.findByText("d-0")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("From"), { target: { value: "2026-08-01" } });
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 1, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 3, 0, 0, 0, 0).getTime(),
        0,
        50,
      ),
    );
    expect(await screen.findByText("e-0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "1" })).toHaveAttribute("aria-pressed", "true");
  });

  it("falls back to the last valid page when the total shrinks after refresh", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const makeRows = (tag: string) =>
      Array.from({ length: 50 }, (_, i) => ({
        ts: `2026-08-02T10:00:${String(i).padStart(2, "0")}Z`,
        provider: "hyb",
        model: `${tag}-${i}`,
        ok: true,
        status: 200,
        error: null,
      }));
    statsMock
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows("a") })
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 250, recentRequests: makeRows("b") })
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 120, recentRequests: makeRows("c") })
      .mockResolvedValueOnce({ ...fullStats(), recentRequestTotal: 120, recentRequests: makeRows("d") });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    expect(await screen.findByText("a-0")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "5" }));
    expect(await screen.findByText("b-0")).toBeInTheDocument();
    expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 4, 50);

    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() =>
      expect(statsMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 2, 50),
    );
    expect(await screen.findByText("d-0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "1" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "4" })).not.toBeInTheDocument();
  });

  it("does not render pagination controls on empty windows or when detail totals are absent", async () => {
    statsMock.mockResolvedValue({
      totalRequests: 0,
      okRequests: 0,
      failedRequests: 0,
      successRate: "0%",
      byProvider: {},
      totalTokens: { input: 0, output: 0, total: 0 },
      cacheHitRate: "-",
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText(/No request data yet/);
    expect(screen.queryByText(/rows/)).not.toBeInTheDocument();

    cleanup();
    statsMock.mockResolvedValue({
      ...fullStats(),
      recentRequests: [
        {
          ts: "2026-08-02T10:00:00Z",
          provider: "hyb",
          model: "m1",
          ok: true,
          status: 200,
          error: null,
        },
      ],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("Request details");
    expect(screen.queryByText(/rows/)).not.toBeInTheDocument();
  });

  it("shows the empty hint when the conversation window has no data", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(convPage());
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    expect(await screen.findByText("No conversation data in this range.")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "By conversation" })).not.toBeInTheDocument();
  });

  it("switches conversation presets and sends window params", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(convPage([{ conversationId: "c1", requests: 1, inputTokens: 10, outputTokens: 1, cachedTokens: 0, reasoningTokens: 0 }]));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    await screen.findByRole("table", { name: "By conversation" });
    convMock.mockClear();

    // The main row also has a 24h/7d button; the conversation row is second.
    fireEvent.click(screen.getAllByRole("button", { name: "24h" })[1]);
    await waitFor(() =>
      expect(convMock).toHaveBeenLastCalledWith(
        "last24h",
        FIXED_NOW - 24 * 3600 * 1000,
        FIXED_NOW,
        0,
        50,
      ),
    );

    fireEvent.click(screen.getAllByRole("button", { name: "7d" })[1]);
    await waitFor(() =>
      expect(convMock).toHaveBeenLastCalledWith(
        "last7d",
        FIXED_NOW - 7 * 24 * 3600 * 1000,
        FIXED_NOW,
        0,
        50,
      ),
    );
  });

  it("All-time omits the window params and pages the full history", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(convPage([{ conversationId: "c1", requests: 1, inputTokens: 10, outputTokens: 1, cachedTokens: 0, reasoningTokens: 0 }]));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    await screen.findByRole("table", { name: "By conversation" });
    convMock.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "All-time" }));
    await waitFor(() =>
      expect(convMock).toHaveBeenLastCalledWith("all", null, null, 0, 50),
    );
    expect(screen.getByRole("button", { name: "All-time" })).toHaveAttribute("aria-pressed", "true");
  });

  it("pages the conversation table with prev/next and page buttons", async () => {
    const makeConv = (n: number) =>
      Array.from({ length: n }, (_, i) => ({
        conversationId: `conv-${i}`,
        requests: 1,
        inputTokens: 10,
        outputTokens: 1,
        lastActive: `2026-08-02T10:00:0${i}Z`,
      }));
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValueOnce({ conversations: makeConv(50), total: 120 });
    convMock.mockResolvedValueOnce({ conversations: makeConv(50), total: 120 });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const table = await screen.findByRole("table", { name: "By conversation" });
    expect(within(table).getByText("conv-0")).toBeInTheDocument();
    expect(screen.getByText("120 rows")).toBeInTheDocument();

    expect(screen.getByRole("button", { name: "Previous conversation page" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Next conversation page" }));
    await waitFor(() =>
      expect(convMock).toHaveBeenLastCalledWith("today", expect.any(Number), expect.any(Number), 1, 50),
    );
    expect(await screen.findByText("conv-0")).toBeInTheDocument();
  });

  it("conversation custom range defaults to today, validates dates and requests with bounds", async () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(convPage([{ conversationId: "c1", requests: 1, inputTokens: 10, outputTokens: 1, cachedTokens: 0, reasoningTokens: 0 }]));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    await screen.findByRole("table", { name: "By conversation" });
    convMock.mockClear();

    fireEvent.click(screen.getAllByRole("button", { name: "Custom" })[1]);
    await waitFor(() =>
      expect(convMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 2, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 3, 0, 0, 0, 0).getTime(),
        0,
        50,
      ),
    );

    fireEvent.change(screen.getByLabelText("Conversation from"), { target: { value: "2026-08-01" } });
    await waitFor(() =>
      expect(convMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 1, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 3, 0, 0, 0, 0).getTime(),
        0,
        50,
      ),
    );

    convMock.mockClear();
    fireEvent.change(screen.getByLabelText("Conversation to"), { target: { value: "2026-07-31" } });
    expect(screen.getByText("End must be on or after start")).toBeInTheDocument();
    expect(convMock).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText("Conversation to"), { target: { value: "2026-08-04" } });
    await waitFor(() =>
      expect(convMock).toHaveBeenLastCalledWith(
        "custom",
        new Date(2026, 7, 1, 0, 0, 0, 0).getTime(),
        new Date(2026, 7, 5, 0, 0, 0, 0).getTime(),
        0,
        50,
      ),
    );
    expect(screen.queryByText("End must be on or after start")).not.toBeInTheDocument();
  });

  it("shows the conversation id/name in each request-details row", async () => {
    statsMock.mockResolvedValue({
      ...fullStats(),
      recentRequests: [
        {
          ts: "2026-08-02T10:00:00Z",
          provider: "hyb",
          model: "m1",
          ok: true,
          status: 200,
          error: null,
          conversationId: "conv-a1b2c3d4e5f6g7h8i9",
          conversationName: "My chat",
        },
        {
          ts: "2026-08-02T10:01:00Z",
          provider: "hyb",
          model: "m2",
          ok: true,
          status: 200,
          error: null,
          conversationId: "conv-z9y8x7w6v5u4t3s2r1q0",
        },
        {
          ts: "2026-08-02T10:02:00Z",
          provider: "hyb",
          model: "m3",
          ok: true,
          status: 200,
          error: null,
        },
      ],
    });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);

    const table = await screen.findByRole("table", { name: "Request details" });
    const rows = within(table).getAllByRole("row");
    expect(within(rows[1]).getByText("My chat")).toBeInTheDocument();
    expect(within(rows[1]).queryByText("conv-a1b2c3d…")).not.toBeInTheDocument();
    const short = within(rows[2]).getByText("conv-z9y8x7w…");
    expect(short).toHaveAttribute("title", "conv-z9y8x7w6v5u4t3s2r1q0");
    const bareCells = within(rows[3]).getAllByRole("cell");
    expect(bareCells[1]).toHaveTextContent("-");
  });

  it("expands a conversation to load and show its requests, and collapses again", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(
      convPage([{ conversationId: "conv-a", requests: 2, inputTokens: 100, outputTokens: 10, cachedTokens: 0, reasoningTokens: 0, lastActive: "2026-08-02T10:00:00Z" }]),
    );
    convReqMock.mockResolvedValue(
      reqPage([
        { ts: "2026-08-02T10:00:00Z", provider: "hyb", model: "m1", ok: true, status: 200, error: null, conversationId: "conv-a" },
        { ts: "2026-08-02T10:00:01Z", provider: "hyb", model: "m2", ok: false, status: 429, error: "rate limited", conversationId: "conv-a" },
      ]),
    );
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const table = await screen.findByRole("table", { name: "By conversation" });
    fireEvent.click(screen.getByRole("button", { name: /Expand conversation conv-a/ }));
    expect(convReqMock).toHaveBeenCalledWith("conv-a", 0, 50);
    const sub = await screen.findByRole("table", { name: "Requests of conv-a" });
    const subRows = within(sub).getAllByRole("row");
    expect(within(subRows[1]).getByText("m1")).toBeInTheDocument();
    expect(within(subRows[2]).getByText("429 rate limited")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Expand conversation conv-a/ }));
    expect(screen.queryByRole("table", { name: "Requests of conv-a" })).not.toBeInTheDocument();
  });

  it("expands two conversations independently and paginates each sub-table", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(
      convPage([
        { conversationId: "conv-a", requests: 3, inputTokens: 100, outputTokens: 10, cachedTokens: 0, reasoningTokens: 0, lastActive: "2026-08-02T10:00:00Z" },
        { conversationId: "conv-b", requests: 2, inputTokens: 50, outputTokens: 5, cachedTokens: 0, reasoningTokens: 0, lastActive: "2026-08-02T10:01:00Z" },
      ]),
    );
    const makeReq = (tag: string) =>
      Array.from({ length: 50 }, (_, i) => ({
        ts: `2026-08-02T10:00:${String(i).padStart(2, "0")}Z`,
        provider: "hyb",
        model: `${tag}-${i}`,
        ok: true,
        status: 200,
        error: null,
        conversationId: tag,
      }));
    convReqMock
      .mockResolvedValueOnce({ requests: makeReq("a"), total: 120 })
      .mockResolvedValueOnce({ requests: makeReq("b"), total: 2 })
      .mockResolvedValueOnce({ requests: makeReq("a2"), total: 120 });
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    await screen.findByRole("table", { name: "By conversation" });

    fireEvent.click(screen.getByRole("button", { name: /Expand conversation conv-a/ }));
    const aTable = await screen.findByRole("table", { name: "Requests of conv-a" });
    expect(within(aTable).getByText("a-0")).toBeInTheDocument();
    expect(screen.getByText("120 rows")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Expand conversation conv-b/ }));
    const bTable = await screen.findByRole("table", { name: "Requests of conv-b" });
    expect(within(bTable).getByText("b-0")).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Requests of conv-a" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next request page" }));
    await waitFor(() => expect(convReqMock).toHaveBeenLastCalledWith("conv-a", 1, 50));
    expect(await screen.findByText("a2-0")).toBeInTheDocument();
    expect(within(screen.getByRole("table", { name: "Requests of conv-b" })).getByText("b-0")).toBeInTheDocument();
  });

  it("shows an error state when loading conversation requests fails", async () => {
    statsMock.mockResolvedValue(fullStats());
    convMock.mockResolvedValue(
      convPage([{ conversationId: "conv-a", requests: 1, inputTokens: 10, outputTokens: 1, cachedTokens: 0, reasoningTokens: 0, lastActive: "2026-08-02T10:00:00Z" }]),
    );
    convReqMock.mockRejectedValue(new Error("boom"));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("363.5K");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    await screen.findByRole("table", { name: "By conversation" });
    fireEvent.click(screen.getByRole("button", { name: /Expand conversation conv-a/ }));

    expect(await screen.findByText("Failed to load conversation requests.")).toBeInTheDocument();
  });

  it("flags cache rates below 50% in red in the request and conversation tables", async () => {
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
          promptTokens: 100,
          completionTokens: 50,
          cachedTokens: 30,
          reasoningTokens: 10,
          totalTokens: 150,
          cacheRate: "30.0%",
          conversationId: "conv-low",
        },
        {
          ts: "2026-08-02T11:00:00Z",
          provider: "hyb",
          model: "deepseek-chat",
          ok: true,
          status: 200,
          error: null,
          promptTokens: 200,
          completionTokens: 50,
          cachedTokens: 180,
          reasoningTokens: 0,
          totalTokens: 250,
          cacheRate: "90.0%",
          conversationId: "conv-high",
        },
      ],
    });
    convMock.mockResolvedValue(
      convPage([
        { conversationId: "conv-low", requests: 1, inputTokens: 100, outputTokens: 50, cachedTokens: 30, reasoningTokens: 10, lastActive: "2026-08-02T10:00:00Z", cacheRate: "30.0%" },
        { conversationId: "conv-high", requests: 1, inputTokens: 200, outputTokens: 50, cachedTokens: 180, reasoningTokens: 0, lastActive: "2026-08-02T11:00:00Z", cacheRate: "90.0%" },
      ]),
    );
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("Request details");
    const reqTable = screen.getByRole("table", { name: "Request details" });
    const lowRow = within(reqTable).getByText("30.0%");
    expect(lowRow.className).toContain("text-red-300");
    const highRow = within(reqTable).getByText("90.0%");
    expect(highRow.className).not.toContain("text-red-300");

    fireEvent.click(screen.getByRole("button", { name: /By conversation/ }));
    const convTable = await screen.findByRole("table", { name: "By conversation" });
    expect(within(convTable).getByText("30.0%").className).toContain("text-red-300");
    expect(within(convTable).getByText("90.0%").className).not.toContain("text-red-300");
  });

  it("copies the full conversation id when a session cell is clicked", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
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
          promptTokens: 100,
          completionTokens: 50,
          cachedTokens: 30,
          reasoningTokens: 10,
          totalTokens: 150,
          cacheRate: "30.0%",
          conversationId: "conv-abc-123",
          conversationName: "my chat",
        },
      ],
    });
    convMock.mockResolvedValue(convPage([]));
    render(<StatsPanel state={{} as never} refresh={async () => {}} />);
    await screen.findByText("Request details");
    fireEvent.click(screen.getByRole("button", { name: /Copy conversation conv-abc-123/ }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("conv-abc-123"));
  });
});
