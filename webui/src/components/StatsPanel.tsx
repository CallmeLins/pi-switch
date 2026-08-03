import { useCallback, useEffect, useRef, useState } from "react";
import type { AppState, RecentRequest, UsageStats } from "../types";
import { api, logsExportUrl } from "../api";
import { Button, Card, Input, SectionTitle } from "./ui";
import { formatCost, formatRequestTime, formatRequestToken, formatTokenCount, formatTokenDimension, formatTotalTokens, shortConversationId } from "../lib/format";
import { computeStatsWindow, todayString } from "../lib/statsWindow";
import type { StatsRange } from "../lib/statsWindow";

const PRESETS: { key: StatsRange; label: string }[] = [
  { key: "today", label: "Today" },
  { key: "last24h", label: "24h" },
  { key: "last7d", label: "7d" },
  { key: "custom", label: "Custom" },
];

const PAGE_SIZES = [50, 100, 200, 500];

// Auto-refresh tiers in milliseconds; `null` means polling is off.
const REFRESH_TIERS: { label: string; ms: number | null }[] = [
  { label: "Off", ms: null },
  { label: "5s", ms: 5000 },
  { label: "30s", ms: 30_000 },
  { label: "5min", ms: 300_000 },
];

export function StatsPanel(_: { state: AppState; refresh: () => Promise<void> }) {
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [range, setRange] = useState<StatsRange>("today");
  const [customFrom, setCustomFrom] = useState("");
  const [customTo, setCustomTo] = useState("");
  const [customError, setCustomError] = useState<string | null>(null);
  const [refreshMs, setRefreshMs] = useState<number | null>(null);
  const [conversationsOpen, setConversationsOpen] = useState(false);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);

  const seq = useRef(0);
  const load = useCallback(
    async (
      range: StatsRange,
      from: number,
      to: number,
      page: number,
      pageSize: number,
      keepOnError = false,
    ) => {
      const id = ++seq.current;
      try {
        const next = await api.stats(range, from, to, page, pageSize);
        if (id === seq.current) {
          const lastPage =
            next.recentRequestTotal != null && next.recentRequestTotal > 0
              ? Math.ceil(next.recentRequestTotal / pageSize) - 1
              : 0;
          if (page > lastPage) {
            // The rolling window shrank the page count while we were on a later
            // page: clamp to the last valid page and re-request (guarded so a
            // clamped page can never re-trigger the clamp).
            setPage(lastPage);
            void load(range, from, to, lastPage, pageSize, keepOnError);
            return;
          }
          setStats(next);
        }
      } catch {
        // A failed auto-refresh keeps the current data instead of blanking the page.
        if (id === seq.current && !keepOnError) {
          setStats(null);
        }
      }
    },
    [],
  );

  useEffect(() => {
    const { from, to } = computeStatsWindow("today", null, null);
    void load("today", from, to, 0, 50);
  }, [load]);

  // Current window bounds for the active range; custom falls back to today.
  const windowBounds = useCallback(
    () =>
      range === "custom"
        ? computeStatsWindow("custom", customFrom || todayString(), customTo || todayString())
        : computeStatsWindow(range, null, null),
    [range, customFrom, customTo],
  );

  // Poll the current window on the selected interval; switching tiers resets
  // the timer (the effect re-runs) and switching back to Off stops it.
  useEffect(() => {
    if (refreshMs == null) {
      return;
    }
    const id = setInterval(() => {
      const { from, to } = windowBounds();
      void load(range, from, to, page, pageSize, true);
    }, refreshMs);
    return () => clearInterval(id);
  }, [refreshMs, range, customFrom, customTo, page, pageSize, load, windowBounds]);
  const select = (key: StatsRange, keepPage = false) => {
    setRange(key);
    if (key === "custom") {
      const from = customFrom || todayString();
      const to = customTo || todayString();
      if (customFrom && customTo && to < from) {
        setCustomError("End must be on or after start");
        return;
      }
      setCustomFrom(from);
      setCustomTo(to);
      setPage(0);
      const { from: f, to: t } = computeStatsWindow("custom", from, to);
      void load("custom", f, t, 0, pageSize);
    } else {
      setCustomError(null);
      const { from, to } = computeStatsWindow(key, null, null);
      if (!keepPage) {
        setPage(0);
      }
      void load(key, from, to, keepPage ? page : 0, pageSize);
    }
  };

  const onCustomDate =
    (which: "from" | "to") => (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value;
      const from = which === "from" ? value : customFrom;
      const to = which === "to" ? value : customTo;
      if (which === "from") {
        setCustomFrom(value);
      } else {
        setCustomTo(value);
      }
      if (!from || !to) {
        setCustomError("Select both start and end dates");
      } else if (to < from) {
        setCustomError("End must be on or after start");
      } else {
        setCustomError(null);
        setPage(0);
        const { from: f, to: t } = computeStatsWindow("custom", from, to);
        void load("custom", f, t, 0, pageSize);
      }
    };

  const byProvider = stats?.byProvider ? Object.entries(stats.byProvider) : [];
  const totals = stats?.totalTokens;
  const totalRows = stats?.recentRequestTotal;
  const totalPages = totalRows != null && totalRows > 0 ? Math.ceil(totalRows / pageSize) : 0;
  const goPage = (nextPage: number) => {
    setPage(nextPage);
    const { from, to } = windowBounds();
    void load(range, from, to, nextPage, pageSize);
  };

  return (
    <div>
      <SectionTitle hint="proxy request usage">Stats</SectionTitle>

      <div className="mb-3 flex flex-wrap items-center gap-2">
        {PRESETS.map(({ key, label }) => (
          <Button
            key={key}
            variant={range === key ? "primary" : "subtle"}
            aria-pressed={range === key}
            onClick={() => select(key)}
          >
            {label}
          </Button>
        ))}
        {range === "custom" && (
          <span className="flex items-center gap-2">
            <Input type="date" aria-label="From" value={customFrom} onChange={onCustomDate("from")} />
            <span className="text-xs text-zinc-500">→</span>
            <Input type="date" aria-label="To" value={customTo} onChange={onCustomDate("to")} />
            {customError && <span className="text-xs text-red-300">{customError}</span>}
          </span>
        )}
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <Button onClick={() => select(range, true)}>Refresh</Button>
        <label className="flex items-center gap-1 text-xs text-zinc-500">
          Auto-refresh
          <select
            aria-label="Auto-refresh"
            value={refreshMs ?? "off"}
            onChange={(e) => setRefreshMs(e.target.value === "off" ? null : Number(e.target.value))}
            className="rounded border border-white/10 bg-zinc-900 px-1.5 py-0.5 text-xs text-zinc-200"
          >
            {REFRESH_TIERS.map(({ label, ms }) => (
              <option key={label} value={ms == null ? "off" : String(ms)}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <a href={logsExportUrl("json")} className="inline-flex">
          <Button>Export JSON</Button>
        </a>
        <a href={logsExportUrl("csv")} className="inline-flex">
          <Button>Export CSV</Button>
        </a>
      </div>

      {!stats || stats.totalRequests === 0 ? (
        <Card>
          <div className="text-sm text-zinc-500">
            No request data yet. Start the proxy and make some requests.
          </div>
        </Card>
      ) : (
        <>
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            <Metric label="Total" value={stats.totalRequests} />
            <Metric label="OK" value={stats.okRequests} tone="green" />
            <Metric label="Failed" value={stats.failedRequests} tone="red" />
            <Metric label="Success" value={stats.successRate} />
            <Metric label="Cache 率" value={stats.cacheHitRate ?? "-"} />
          </div>
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            <Metric label="Input" value={formatTokenDimension(totals?.input)} />
            <Metric label="Output" value={formatTokenDimension(totals?.output)} />
            <Metric label="Cached" value={formatTokenDimension(totals?.cached)} badge="⊆ Input" />
            <Metric
              label="Reasoning"
              value={formatTokenDimension(totals?.reasoning)}
              badge="⊆ Output"
            />
            <Metric label="Total" value={formatTotalTokens(totals)} />
          </div>
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            <Metric label="Cost" value={formatCost(stats.totalCost)} />
            {stats.costUnknown ? (
              <div className="col-span-full text-xs text-zinc-500">
                {stats.costUnknown} unknown cost rows
              </div>
            ) : null}
          </div>
          {stats.avgLatencyMs != null && (
            <div className="mb-4 text-sm text-zinc-400">
              Avg latency: <span className="text-zinc-200">{stats.avgLatencyMs} ms</span>
            </div>
          )}

          {byProvider.length > 0 && (
            <Card>
              <div className="mb-2 text-sm font-semibold text-zinc-200">By provider</div>
              <table className="w-full text-sm">
                <thead className="text-left text-xs text-zinc-500">
                  <tr>
                    <th className="pb-1">Provider</th>
                    <th className="pb-1 text-right">Total</th>
                    <th className="pb-1 text-right">OK</th>
                    <th className="pb-1 text-right">Rate</th>
                    <th className="pb-1 text-right">Tokens</th>
                  </tr>
                </thead>
                <tbody>
                  {byProvider.map(([name, ps]) => {
                    const rate = ps.total > 0 ? Math.round((ps.ok / ps.total) * 100) : 0;
                    return (
                      <tr key={name} className="border-t border-white/5">
                        <td className="py-1 text-zinc-200">{name}</td>
                        <td className="py-1 text-right text-zinc-400">{ps.total}</td>
                        <td className="py-1 text-right text-zinc-400">{ps.ok}</td>
                        <td className="py-1 text-right text-zinc-400">{rate}%</td>
                        <td className="py-1 text-right text-zinc-400">
                          {formatProviderTokens(ps.promptTokens + ps.outputTokens)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </Card>
          )}

          {stats.recentRequests?.length ? (
            <Card className="mt-4">
              <div className="mb-2 text-sm font-semibold text-zinc-200">Request details</div>
              <div className="overflow-x-auto">
                <table aria-label="Request details" className="w-full text-sm">
                  <thead className="text-left text-xs text-zinc-500">
                    <tr>
                      <th className="pb-1 pr-2">Time</th>
                      <th className="pb-1 pr-2">Provider</th>
                      <th className="pb-1 pr-2">Model</th>
                      <th className="pb-1 pr-2">Status</th>
                      <th className="pb-1 pr-2 text-right">Input</th>
                      <th className="pb-1 pr-2 text-right">Output</th>
                      <th className="pb-1 pr-2 text-right">Cached</th>
                      <th className="pb-1 pr-2 text-right">Reasoning</th>
                      <th className="pb-1 pr-2 text-right">Rate</th>
                      <th className="pb-1 text-right">Total</th>
                      <th className="pb-1 text-right">Cost</th>
                    </tr>
                  </thead>
                  <tbody>
                    {stats.recentRequests.map((r, i) => {
                      const status = formatRequestStatus(r);
                      const tokenCols = [
                        ["Input", formatRequestToken(r.promptTokens)],
                        ["Output", formatRequestToken(r.completionTokens)],
                        ["Cached", formatRequestToken(r.cachedTokens)],
                        ["Reasoning", formatRequestToken(r.reasoningTokens)],
                        ["Rate", r.cacheRate ?? "-"],
                        ["Total", formatRequestToken(r.totalTokens)],
                        ["Cost", formatCost(r.cost)],
                      ] as const;
                      return (
                        <tr
                          key={`${r.ts ?? ""}-${r.model ?? ""}-${i}`}
                          className="border-t border-white/5"
                        >
                          <td className="py-1 pr-2 whitespace-nowrap text-zinc-500">
                            {formatRequestTime(r.ts)}
                          </td>
                          <td className="py-1 pr-2 text-zinc-300">{r.provider ?? "-"}</td>
                          <td className="py-1 pr-2 text-zinc-300">{r.model ?? "-"}</td>
                          <td className="py-1 pr-2 text-zinc-400">
                            <span className="block max-w-[14rem] truncate" title={status}>
                              {status}
                            </span>
                          </td>
                          {tokenCols.map(([label, value]) => (
                            <td key={label} className="py-1 pr-2 text-right text-zinc-400">
                              {value}
                            </td>
                          ))}
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
              {totalRows != null && totalRows > 0 && (
                <div className="mt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-zinc-400">
                  <span>{totalRows} rows</span>
                  {totalPages > 1 && (
                    <span className="flex items-center gap-1">
                      <Button
                        aria-label="Previous page"
                        disabled={page === 0}
                        onClick={() => goPage(page - 1)}
                      >
                        ‹
                      </Button>
                      {pageNumbers(page, totalPages).map((n, i) =>
                        n === "…" ? (
                          <span key={`gap-${i}`} className="px-1 text-zinc-600">
                            …
                          </span>
                        ) : (
                          <Button
                            key={n}
                            variant={n - 1 === page ? "primary" : "subtle"}
                            aria-pressed={n - 1 === page}
                            onClick={() => goPage(n - 1)}
                          >
                            {n}
                          </Button>
                        ),
                      )}
                      <Button
                        aria-label="Next page"
                        disabled={page >= totalPages - 1}
                        onClick={() => goPage(page + 1)}
                      >
                        ›
                      </Button>
                    </span>
                  )}
                  <label className="flex items-center gap-1 text-zinc-500">
                    Rows per page
                    <select
                      aria-label="Rows per page"
                      value={pageSize}
                      onChange={(e) => {
                        const next = Number(e.target.value);
                        setPageSize(next);
                        setPage(0);
                        const { from, to } = windowBounds();
                        void load(range, from, to, 0, next);
                      }}
                      className="rounded border border-white/10 bg-zinc-900 px-1.5 py-0.5 text-xs text-zinc-200"
                    >
                      {PAGE_SIZES.map((s) => (
                        <option key={s} value={s}>
                          {s}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
              )}
            </Card>
          ) : null}

          {stats.byConversation?.length ? (
            <Card className="mt-4">
              <button
                type="button"
                aria-expanded={conversationsOpen}
                onClick={() => setConversationsOpen((v) => !v)}
                className="mb-2 flex w-full items-center justify-between text-sm font-semibold text-zinc-200"
              >
                <span>By conversation</span>
                <span className="text-zinc-500">{conversationsOpen ? "▾" : "▸"}</span>
              </button>
              {conversationsOpen && (
                <div className="divide-y divide-white/5">
                  {stats.byConversation.map((c) => (
                    <div key={c.conversationId} className="py-1.5 text-sm">
                      <div className="flex items-center justify-between gap-3">
                        <span className="truncate text-zinc-200" title={c.conversationId}>
                          {c.name || shortConversationId(c.conversationId)}
                        </span>
                        <span className="text-zinc-500">{c.requests} requests</span>
                      </div>
                      <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-zinc-400">
                        <span>Input {formatTokenDimension(c.inputTokens)}</span>
                        <span>Output {formatTokenDimension(c.outputTokens)}</span>
                        <span>Cached {formatTokenDimension(c.cachedTokens)}</span>
                        <span>Reasoning {formatTokenDimension(c.reasoningTokens)}</span>
                        <span>Rate {c.cacheRate ?? "-"}</span>
                        <span>Total {formatTokenDimension(c.inputTokens + c.outputTokens)}</span>
                        <span>Cost {formatCost(c.cost)}</span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </Card>
          ) : null}
        </>
      )}
    </div>
  );
}

function Metric({
  label,
  value,
  tone = "zinc",
  badge,
}: {
  label: string;
  value: string | number;
  tone?: "zinc" | "green" | "red";
  badge?: string;
}) {
  const color =
    tone === "green" ? "text-emerald-300" : tone === "red" ? "text-red-300" : "text-zinc-100";
  return (
    <Card className="py-3">
      <div className="text-[11px] uppercase tracking-wide text-zinc-500">
        {label}
        {badge && <span className="ml-1 text-[9px] normal-case text-zinc-600">{badge}</span>}
      </div>
      <div className={"mt-1 text-xl font-semibold " + color}>{value}</div>
    </Card>
  );
}

function formatProviderTokens(tokens: number): string {
  if (tokens === 0) {
    return "-";
  }
  return formatTokenCount(tokens);
}

function pageNumbers(current: number, total: number): (number | "…")[] {
  if (total <= 7) {
    return Array.from({ length: total }, (_, i) => i + 1);
  }
  const wanted = new Set(
    [1, total, current, current + 1, current + 2].map((p) => Math.min(Math.max(p, 1), total)),
  );
  const sorted = [...wanted].sort((a, b) => a - b);
  const out: (number | "…")[] = [];
  let prev = 0;
  for (const p of sorted) {
    if (p - prev > 1) {
      out.push("…");
    }
    out.push(p);
    prev = p;
  }
  return out;
}

function formatRequestStatus(r: RecentRequest): string {
  if (r.ok) {
    return r.status != null ? String(r.status) : "ok";
  }
  const parts = [r.status != null ? String(r.status) : null, r.error ?? null].filter(Boolean);
  return parts.join(" ") || "failed";
}
