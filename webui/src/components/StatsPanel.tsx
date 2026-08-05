import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import type { AppState, ConversationRequestsPage, ConversationStats, ConversationsPage, RecentRequest, UsageStats } from "../types";
import { api, logsExportUrl } from "../api";
import { Button, Card, Input, SectionTitle } from "./ui";
import { decodeConversationName, formatCost, formatRequestTime, formatRequestToken, formatTokenCount, formatTokenDimension, formatTotalTokens, isLowCacheRate, shortConversationId } from "../lib/format";
import { computeConversationWindow, computeStatsWindow, todayString } from "../lib/statsWindow";
import type { ConversationRange, StatsRange } from "../lib/statsWindow";
import { useI18n } from "../i18n";

const PRESET_KEYS: StatsRange[] = ["today", "last24h", "last7d", "custom"];

const CONV_PRESETS: { key: ConversationRange; label: string }[] = [
  { key: "today", label: "Today" },
  { key: "last24h", label: "24h" },
  { key: "last7d", label: "7d" },
  { key: "custom", label: "Custom" },
  { key: "all", label: "All-time" },
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
  const { t } = useI18n();
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [range, setRange] = useState<StatsRange>("today");
  const [customFrom, setCustomFrom] = useState("");
  const [customTo, setCustomTo] = useState("");
  const [customError, setCustomError] = useState<string | null>(null);
  const [refreshMs, setRefreshMs] = useState<number | null>(null);
  const [conversationsOpen, setConversationsOpen] = useState(false);
  const [requestsOpen, setRequestsOpen] = useState(true);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [convRange, setConvRange] = useState<ConversationRange>("today");
  const [convFrom, setConvFrom] = useState("");
  const [convTo, setConvTo] = useState("");
  const [convError, setConvError] = useState<string | null>(null);
  const [convPage, setConvPage] = useState(0);
  const [convPageSize, setConvPageSize] = useState(50);
  const [convData, setConvData] = useState<ConversationsPage | null>(null);
  const [expandedConvs, setExpandedConvs] = useState<Set<string>>(new Set());
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

  const convSeq = useRef(0);
  const loadConversations = useCallback(
    async (
      range: ConversationRange,
      from: number | null,
      to: number | null,
      page: number,
      pageSize: number,
      keepOnError = false,
    ) => {
      const id = ++convSeq.current;
      try {
        const next = await api.statsConversations(range, from, to, page, pageSize);
        if (id === convSeq.current) {
          const lastPage = next.total > 0 ? Math.ceil(next.total / pageSize) - 1 : 0;
          if (page > lastPage) {
            // Same clamp semantics as the main load: a shrunken window can
            // invalidate the current page, so clamp and re-request once.
            setConvPage(lastPage);
            void loadConversations(range, from, to, lastPage, pageSize, keepOnError);
            return;
          }
          setConvData(next);
        }
      } catch {
        if (id === convSeq.current && !keepOnError) {
          setConvData(null);
        }
      }
    },
    [],
  );

  useEffect(() => {
    const { from, to } = computeStatsWindow("today", null, null);
    void load("today", from, to, 0, 50);
    const conv = computeConversationWindow("today", null, null);
    void loadConversations("today", conv.from, conv.to, 0, 50);
  }, [load, loadConversations]);

  // Current window bounds for the active range; custom falls back to today.
  const windowBounds = useCallback(
    () =>
      range === "custom"
        ? computeStatsWindow("custom", customFrom || todayString(), customTo || todayString())
        : computeStatsWindow(range, null, null),
    [range, customFrom, customTo],
  );

  // Independent window bounds for the conversation browser; "all" is a null
  // window (full history).
  const convWindowBounds = useCallback(
    () =>
      convRange === "custom"
        ? computeConversationWindow("custom", convFrom || todayString(), convTo || todayString())
        : computeConversationWindow(convRange, null, null),
    [convRange, convFrom, convTo],
  );

  // Poll the current window on the selected interval; switching tiers resets
  // the timer (the effect re-runs) and switching back to Off stops it.
  // Auto-refresh refreshes both windows, each with its own bounds.
  useEffect(() => {
    if (refreshMs == null) {
      return;
    }
    const id = setInterval(() => {
      const { from, to } = windowBounds();
      void load(range, from, to, page, pageSize, true);
      const conv = convWindowBounds();
      void loadConversations(convRange, conv.from, conv.to, convPage, convPageSize, true);
    }, refreshMs);
    return () => clearInterval(id);
  }, [
    refreshMs,
    range,
    customFrom,
    customTo,
    page,
    pageSize,
    load,
    windowBounds,
    convRange,
    convFrom,
    convTo,
    convPage,
    convPageSize,
    loadConversations,
    convWindowBounds,
  ]);
  const select = (key: StatsRange, keepPage = false) => {
    setRange(key);
    if (key === "custom") {
      const from = customFrom || todayString();
      const to = customTo || todayString();
      if (customFrom && customTo && to < from) {
        setCustomError(t("End must be on or after start"));
        return;
      }
      setCustomFrom(from);
      setCustomTo(to);
      setPage(0);
      const { from: f, to: toMs } = computeStatsWindow("custom", from, to);
      void load("custom", f, toMs, 0, pageSize);
    } else {
      setCustomError(null);
      const { from, to } = computeStatsWindow(key, null, null);
      if (!keepPage) {
        setPage(0);
      }
      void load(key, from, to, keepPage ? page : 0, pageSize);
    }
  };

  const convSelect = (key: ConversationRange) => {
    setConvRange(key);
    if (key === "custom") {
      const from = convFrom || todayString();
      const to = convTo || todayString();
      if (convFrom && convTo && to < from) {
        setConvError("End must be on or after start");
        return;
      }
      setConvFrom(from);
      setConvTo(to);
      setConvPage(0);
      const { from: f, to: t } = computeConversationWindow("custom", from, to);
      void loadConversations("custom", f, t, 0, convPageSize);
    } else {
      setConvError(null);
      const { from, to } = computeConversationWindow(key, null, null);
      setConvPage(0);
      void loadConversations(key, from, to, 0, convPageSize);
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
        setCustomError(t("Select both start and end dates"));
      } else if (to < from) {
        setCustomError(t("End must be on or after start"));
      } else {
        setCustomError(null);
        setPage(0);
        const { from: f, to: toMs } = computeStatsWindow("custom", from, to);
        void load("custom", f, toMs, 0, pageSize);
      }
    };

  const onConvCustomDate =
    (which: "from" | "to") => (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value;
      const from = which === "from" ? value : convFrom;
      const to = which === "to" ? value : convTo;
      if (which === "from") {
        setConvFrom(value);
      } else {
        setConvTo(value);
      }
      if (!from || !to) {
        setConvError("Select both start and end dates");
      } else if (to < from) {
        setConvError("End must be on or after start");
      } else {
        setConvError(null);
        setConvPage(0);
        const { from: f, to: toMs } = computeConversationWindow("custom", from, to);
        void loadConversations("custom", f, toMs, 0, convPageSize);
      }
    };

  const PRESETS: { key: StatsRange; label: string }[] = [
    { key: "today", label: t("Today") },
    { key: "last24h", label: t("24h") },
    { key: "last7d", label: t("7d") },
    { key: "custom", label: t("Custom") },
  ];

  const byProvider = stats?.byProvider ? Object.entries(stats.byProvider) : [];
  const totals = stats?.totalTokens;
  const totalRows = stats?.recentRequestTotal;
  const totalPages = totalRows != null && totalRows > 0 ? Math.ceil(totalRows / pageSize) : 0;
  const goPage = (nextPage: number) => {
    setPage(nextPage);
    const { from, to } = windowBounds();
    void load(range, from, to, nextPage, pageSize);
  };
  const convTotalPages =
    convData && convData.total > 0 ? Math.ceil(convData.total / convPageSize) : 0;
  const convGoPage = (nextPage: number) => {
    setConvPage(nextPage);
    const { from, to } = convWindowBounds();
    void loadConversations(convRange, from, to, nextPage, convPageSize);
  };
  const toggleConv = (id: string) => {
    setExpandedConvs((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  return (
    <div>
      <SectionTitle hint={t("proxy request usage")}>{t("Stats")}</SectionTitle>

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
            <Input type="date" aria-label={t("From")} value={customFrom} onChange={onCustomDate("from")} />
            <span className="text-xs text-zinc-500">→</span>
            <Input type="date" aria-label={t("To")} value={customTo} onChange={onCustomDate("to")} />
            {customError && <span className="text-xs text-red-300">{customError}</span>}
          </span>
        )}
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <Button onClick={() => select(range, true)}>{t("Refresh")}</Button>
        <label className="flex items-center gap-1 text-xs text-zinc-500">
          Auto-refresh
          <select
            aria-label={t("Auto-refresh")}
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
          <Button>{t("Export JSON")}</Button>
        </a>
        <a href={logsExportUrl("csv")} className="inline-flex">
          <Button>{t("Export CSV")}</Button>
        </a>
      </div>

      {!stats || stats.totalRequests === 0 ? (
        <Card>
          <div className="text-sm text-zinc-500">
            {t("No request data yet. Start the proxy and make some requests.")}
          </div>
        </Card>
      ) : (
        <>
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            <Metric label={t("Total")} value={stats.totalRequests} />
            <Metric label={t("OK")} value={stats.okRequests} tone="green" />
            <Metric label={t("Failed")} value={stats.failedRequests} tone="red" />
            <Metric label={t("Success")} value={stats.successRate} />
            <Metric label={t("Cache rate")} value={stats.cacheHitRate ?? "-"} />
          </div>
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            <Metric label={t("Input")} value={formatTokenDimension(totals?.input)} />
            <Metric label={t("Output")} value={formatTokenDimension(totals?.output)} />
            <Metric label={t("Cached")} value={formatTokenDimension(totals?.cached)} badge="⊆ Input" />
            <Metric
              label={t("Reasoning")}
              value={formatTokenDimension(totals?.reasoning)}
              badge="⊆ Output"
            />
            <Metric label={t("Total")} value={formatTotalTokens(totals)} />
          </div>
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            <Metric label="Cost" value={formatCost(stats.totalCost)} />
            {stats.costUnknown ? (
              <div className="col-span-full text-xs text-zinc-500">
                {stats.costUnknown} {t("unknown cost rows")}
              </div>
            ) : null}
          </div>
          {stats.avgLatencyMs != null && (
            <div className="mb-4 text-sm text-zinc-400">
              {t("Avg latency:")} <span className="text-zinc-200">{stats.avgLatencyMs} ms</span>
            </div>
          )}

          {byProvider.length > 0 && (
            <Card>
              <div className="mb-2 text-sm font-semibold text-zinc-200">{t("By provider")}</div>
              <table className="w-full text-sm">
                <thead className="text-left text-xs text-zinc-500">
                  <tr>
                    <th className="pb-1">{t("Provider")}</th>
                    <th className="pb-1 text-right">{t("Total")}</th>
                    <th className="pb-1 text-right">{t("OK")}</th>
                    <th className="pb-1 text-right">{t("Rate")}</th>
                    <th className="pb-1 text-right">{t("Tokens")}</th>
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
              <button
                type="button"
                aria-expanded={requestsOpen}
                onClick={() => setRequestsOpen((v) => !v)}
                className="mb-2 flex w-full items-center justify-between text-sm font-semibold text-zinc-200"
              >
                <span>{t("Request details")}</span>
                <span className="text-zinc-500">{requestsOpen ? "▾" : "▸"}</span>
              </button>
              {requestsOpen && (
                <>
                  <div className="overflow-x-auto">
                <table aria-label={t("Request details")} className="w-full text-sm">
                  <thead className="text-left text-xs text-zinc-500">
                    <tr>
                      <th className="pb-1 pr-2">{t("Time")}</th>
                      <th className="pb-1 pr-2">{t("Session")}</th>
                      <th className="pb-1 pr-2">{t("Provider")}</th>
                      <th className="pb-1 pr-2">{t("Model")}</th>
                      <th className="pb-1 pr-2">{t("Status")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Input")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Output")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Cached")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Reasoning")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Cache rate")}</th>
                      <th className="pb-1 text-right">{t("Total")}</th>
                      <th className="pb-1 text-right">{t("Cost")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {stats.recentRequests.map((r, i) => (
                      <RequestRow key={`${r.ts ?? ""}-${r.model ?? ""}-${i}`} r={r} i={i} />
                    ))}
                  </tbody>
                </table>
              </div>
              {totalRows != null && totalRows > 0 && (
                <div className="mt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-zinc-400">
                  <span>{totalRows} {t("rows")}</span>
                  {totalPages > 1 && (
                    <span className="flex items-center gap-1">
                      <Button
                        aria-label={t("Previous page")}
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
                        aria-label={t("Next page")}
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
                      aria-label={t("Rows per page")}
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
                </>
              )}
            </Card>
          ) : null}

          <Card className="mt-4">
            <button
              type="button"
              aria-expanded={conversationsOpen}
              onClick={() => setConversationsOpen((v) => !v)}
              className="mb-2 flex w-full items-center justify-between text-sm font-semibold text-zinc-200"
            >
              <span>{t("By conversation")}</span>
              <span className="text-zinc-500">{conversationsOpen ? "▾" : "▸"}</span>
            </button>
            {conversationsOpen && (
              <div>
                <div className="mb-3 flex flex-wrap items-center gap-2">
                  {CONV_PRESETS.map(({ key, label }) => (
                    <Button
                      key={key}
                      variant={convRange === key ? "primary" : "subtle"}
                      aria-pressed={convRange === key}
                      onClick={() => convSelect(key)}
                    >
                      {label}
                    </Button>
                  ))}
                  {convRange === "custom" && (
                    <span className="flex items-center gap-2">
                      <Input type="date" aria-label={t("Conversation from")} value={convFrom} onChange={onConvCustomDate("from")} />
                      <span className="text-xs text-zinc-500">→</span>
                      <Input type="date" aria-label={t("Conversation to")} value={convTo} onChange={onConvCustomDate("to")} />
                      {convError && <span className="text-xs text-red-300">{convError}</span>}
                    </span>
                  )}
                </div>
                {!convData || convData.total === 0 ? (
                  <div className="text-sm text-zinc-500">{t("No conversation data in this range.")}</div>
                ) : (
                  <>
                    <div className="overflow-x-auto">
                      <table aria-label={t("By conversation")} className="w-full text-sm">
                        <thead className="text-left text-xs text-zinc-500">
                          <tr>
                            <th className="pb-1 pr-2"></th>
                            <th className="pb-1 pr-2">{t("Time")}</th>
                            <th className="pb-1 pr-2">{t("Session")}</th>
                            <th className="pb-1 pr-2 text-right">{t("Requests")}</th>
                            <th className="pb-1 pr-2 text-right">{t("Input")}</th>
                            <th className="pb-1 pr-2 text-right">{t("Output")}</th>
                            <th className="pb-1 pr-2 text-right">{t("Cached")}</th>
                            <th className="pb-1 pr-2 text-right">{t("Reasoning")}</th>
                            <th className="pb-1 pr-2 text-right">{t("Cache rate")}</th>
                            <th className="pb-1 pr-2 text-right">{t("Total")}</th>
                            <th className="pb-1 text-right">{t("Cost")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {convData.conversations.map((c) => (
                            <Fragment key={c.conversationId}>
                              <tr className="border-t border-white/5">
                                <td className="py-1 pr-2">
                                  <button
                                    type="button"
                                    aria-expanded={expandedConvs.has(c.conversationId)}
                                    aria-label={`Expand conversation ${c.conversationId}`}
                                    onClick={() => toggleConv(c.conversationId)}
                                    className="text-zinc-500 hover:text-zinc-200"
                                  >
                                    {expandedConvs.has(c.conversationId) ? "▾" : "▸"}
                                  </button>
                                </td>
                                <td className="py-1 pr-2 whitespace-nowrap text-zinc-500">
                                  {formatRequestTime(c.lastActive)}
                                </td>
                                <td className="py-1 pr-2">
                                  <CopyableSessionCell id={c.conversationId} name={c.name} className="max-w-[14rem]" />
                                </td>
                                <td className="py-1 pr-2 text-right text-zinc-400">{c.requests}</td>
                                <td className="py-1 pr-2 text-right text-zinc-400">
                                  {formatRequestToken(c.inputTokens)}
                                </td>
                                <td className="py-1 pr-2 text-right text-zinc-400">
                                  {formatRequestToken(c.outputTokens)}
                                </td>
                                <td className="py-1 pr-2 text-right text-zinc-400">
                                  {formatRequestToken(c.cachedTokens)}
                                </td>
                                <td className="py-1 pr-2 text-right text-zinc-400">
                                  {formatRequestToken(c.reasoningTokens)}
                                </td>
                                <td className={`py-1 pr-2 text-right ${isLowCacheRate(c.cacheRate) ? "text-red-300" : "text-zinc-400"}`}>{c.cacheRate ?? "-"}</td>
                                <td className="py-1 pr-2 text-right text-zinc-400">
                                  {formatRequestToken(c.inputTokens + c.outputTokens)}
                                </td>
                                <td className="py-1 text-right text-zinc-400">{formatCost(c.cost)}</td>
                              </tr>
                              {expandedConvs.has(c.conversationId) && (
                                <tr className="border-t border-white/5">
                                  <td colSpan={11} className="py-2 pl-8 pr-2">
                                    <ExpandedConversationRequests conv={c} />
                                  </td>
                                </tr>
                              )}
                            </Fragment>
                          ))}
                        </tbody>
                      </table>
                    </div>
                    <div className="mt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-zinc-400">
                      <span>{convData.total} {t("rows")}</span>
                      {convTotalPages > 1 && (
                        <span className="flex items-center gap-1">
                          <Button
                            aria-label={t("Previous conversation page")}
                            disabled={convPage === 0}
                            onClick={() => convGoPage(convPage - 1)}
                          >
                            ‹
                          </Button>
                          {pageNumbers(convPage, convTotalPages).map((n, i) =>
                            n === "…" ? (
                              <span key={`gap-${i}`} className="px-1 text-zinc-600">
                                …
                              </span>
                            ) : (
                              <Button
                                key={n}
                                variant={n - 1 === convPage ? "primary" : "subtle"}
                                aria-pressed={n - 1 === convPage}
                                onClick={() => convGoPage(n - 1)}
                              >
                                {n}
                              </Button>
                            ),
                          )}
                          <Button
                            aria-label={t("Next conversation page")}
                            disabled={convPage >= convTotalPages - 1}
                            onClick={() => convGoPage(convPage + 1)}
                          >
                            ›
                          </Button>
                        </span>
                      )}
                      <label className="flex items-center gap-1 text-zinc-500">
                        Rows per page
                        <select
                          aria-label={t("Conversation rows per page")}
                          value={convPageSize}
                          onChange={(e) => {
                            const next = Number(e.target.value);
                            setConvPageSize(next);
                            setConvPage(0);
                            const { from, to } = convWindowBounds();
                            void loadConversations(convRange, from, to, 0, next);
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
                  </>
                )}
              </div>
            )}
          </Card>
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

// ─── Request detail row (shared by the stats page and the expanded
// ─── conversation browser so both render identically) ─────────

function RequestRow({ r, i }: { r: RecentRequest; i: number }) {
  const status = formatRequestStatus(r);
  const tokenCols = [
    ["Input", formatRequestToken(r.promptTokens)],
    ["Output", formatRequestToken(r.completionTokens)],
    ["Cached", formatRequestToken(r.cachedTokens)],
    ["Reasoning", formatRequestToken(r.reasoningTokens)],
    ["Cache rate", r.cacheRate ?? "-", isLowCacheRate(r.cacheRate) ? "text-red-300" : undefined],
    ["Total", formatRequestToken(r.totalTokens)],
    ["Cost", formatCost(r.cost)],
  ] as const;
  return (
    <tr className="border-t border-white/5">
      <td className="py-1 pr-2 whitespace-nowrap text-zinc-500">
        {formatRequestTime(r.ts)}
      </td>
      <td className="py-1 pr-2">
        <CopyableSessionCell id={r.conversationId} name={r.conversationName} />
      </td>
      <td className="py-1 pr-2 text-zinc-300">{r.provider ?? "-"}</td>
      <td className="py-1 pr-2 text-zinc-300">{r.model ?? "-"}</td>
      <td className="py-1 pr-2 text-zinc-400">
        <span className="block max-w-[14rem] truncate" title={status}>
          {status}
        </span>
      </td>
      {tokenCols.map(([label, value, tone]) => (
        <td key={label} className={`py-1 pr-2 text-right ${tone ?? "text-zinc-400"}`}>
          {value}
        </td>
      ))}
    </tr>
  );
}

/**
 * Session cell that shows the display name (or a truncated id) and copies
 * the full conversation id to the clipboard on click.
 */
function CopyableSessionCell({
  id,
  name,
  className = "",
}: {
  id?: string | null;
  name?: string | null;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);
  const display = decodeConversationName(name ?? "") || (id ? shortConversationId(id) : "-");
  return (
    <button
      type="button"
      title={id ?? undefined}
      aria-label={id ? `Copy conversation ${id}` : undefined}
      onClick={() => {
        if (!id) return;
        navigator.clipboard
          ?.writeText(id)
          .then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 1200);
          })
          .catch(() => {});
      }}
      className={`block max-w-[12rem] truncate text-left text-zinc-300 hover:text-zinc-100 ${className}`}
    >
      {copied ? "✓" : display}
    </button>
  );
}

// ─── Expanded conversation request browser ────────────────────

function ExpandedConversationRequests({ conv }: { conv: ConversationStats }) {
  const { t } = useI18n();
  const [data, setData] = useState<ConversationRequestsPage | null>(null);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [error, setError] = useState(false);
  const seq = useRef(0);

  const load = useCallback(
    async (page: number, pageSize: number) => {
      const id = ++seq.current;
      setError(false);
      try {
        const next = await api.conversationRequests(conv.conversationId, page, pageSize);
        if (id === seq.current) {
          const lastPage = next.total > 0 ? Math.ceil(next.total / pageSize) - 1 : 0;
          if (page > lastPage) {
            // The conversation shrank while we were on a later page: clamp
            // and re-request once (guarded so it can never re-trigger).
            setPage(lastPage);
            void load(lastPage, pageSize);
            return;
          }
          setData(next);
        }
      } catch {
        if (id === seq.current) {
          setError(true);
        }
      }
    },
    [conv.conversationId],
  );

  useEffect(() => {
    void load(0, 50);
  }, [load]);

  const totalPages = data && data.total > 0 ? Math.ceil(data.total / pageSize) : 0;
  const goPage = (nextPage: number) => {
    setPage(nextPage);
    void load(nextPage, pageSize);
  };

  return (
    <div>
      <div className="mb-1 text-xs text-zinc-500">
        Requests in {decodeConversationName(conv.name ?? "") || shortConversationId(conv.conversationId)}
      </div>
      {error ? (
        <div className="text-sm text-red-300">Failed to load conversation requests.</div>
      ) : !data ? (
        <div className="text-sm text-zinc-500">Loading…</div>
      ) : data.requests.length === 0 ? (
        <div className="text-sm text-zinc-500">No requests in this conversation.</div>
      ) : (
        <>
          <div className="overflow-x-auto">
            <table aria-label={`Requests of ${conv.conversationId}`} className="w-full text-sm">
              <thead className="text-left text-xs text-zinc-500">
                <tr>
                  <th className="pb-1 pr-2">{t("Time")}</th>
                  <th className="pb-1 pr-2">{t("Session")}</th>
                  <th className="pb-1 pr-2">{t("Provider")}</th>
                  <th className="pb-1 pr-2">{t("Model")}</th>
                  <th className="pb-1 pr-2">{t("Status")}</th>
                  <th className="pb-1 pr-2 text-right">{t("Input")}</th>
                  <th className="pb-1 pr-2 text-right">{t("Output")}</th>
                  <th className="pb-1 pr-2 text-right">{t("Cached")}</th>
                  <th className="pb-1 pr-2 text-right">{t("Reasoning")}</th>
                  <th className="pb-1 pr-2 text-right">{t("Cache rate")}</th>
                  <th className="pb-1 text-right">{t("Total")}</th>
                  <th className="pb-1 text-right">{t("Cost")}</th>
                </tr>
              </thead>
              <tbody>
                {data.requests.map((r, i) => (
                  <RequestRow key={`${r.ts ?? ""}-${r.model ?? ""}-${i}`} r={r} i={i} />
                ))}
              </tbody>
            </table>
          </div>
          <div className="mt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-zinc-400">
            <span>{data.total} rows</span>
            {totalPages > 1 && (
              <span className="flex items-center gap-1">
                <Button
                  aria-label="Previous request page"
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
                  aria-label="Next request page"
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
                aria-label="Request rows per page"
                value={pageSize}
                onChange={(e) => {
                  const next = Number(e.target.value);
                  setPageSize(next);
                  setPage(0);
                  void load(0, next);
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
        </>
      )}
    </div>
  );
}
