import { useEffect, useRef, useState } from "react";
import type { AppState, RecentRequest, UsageStats } from "../types";
import { api, logsExportUrl } from "../api";
import { Button, Card, Input, SectionTitle } from "./ui";
import { formatRequestTime, formatRequestToken, formatTokenCount, formatTokenDimension, formatTotalTokens, shortConversationId } from "../lib/format";
import { computeStatsWindow, todayString } from "../lib/statsWindow";
import type { StatsRange } from "../lib/statsWindow";
import { useI18n } from "../i18n";

const PRESET_KEYS: StatsRange[] = ["today", "last24h", "last7d", "custom"];

export function StatsPanel(_: { state: AppState; refresh: () => Promise<void> }) {
  const { t } = useI18n();
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [range, setRange] = useState<StatsRange>("today");
  const [customFrom, setCustomFrom] = useState("");
  const [customTo, setCustomTo] = useState("");
  const [customError, setCustomError] = useState<string | null>(null);

  const seq = useRef(0);
  const load = async (range: StatsRange, from: number, to: number) => {
    const id = ++seq.current;
    try {
      const next = await api.stats(range, from, to);
      if (id === seq.current) {
        setStats(next);
      }
    } catch {
      if (id === seq.current) {
        setStats(null);
      }
    }
  };
  useEffect(() => {
    const { from, to } = computeStatsWindow("today", null, null);
    void load("today", from, to);
  }, []);

  const select = (key: StatsRange) => {
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
      const { from: f, to: toMs } = computeStatsWindow("custom", from, to);
      void load("custom", f, toMs);
    } else {
      setCustomError(null);
      const { from, to } = computeStatsWindow(key, null, null);
      void load(key, from, to);
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
        const { from: f, to: toMs } = computeStatsWindow("custom", from, to);
        void load("custom", f, toMs);
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
            <Input type="date" aria-label="From" value={customFrom} onChange={onCustomDate("from")} />
            <span className="text-xs text-zinc-500">→</span>
            <Input type="date" aria-label="To" value={customTo} onChange={onCustomDate("to")} />
            {customError && <span className="text-xs text-red-300">{customError}</span>}
          </span>
        )}
      </div>

      <div className="mb-3 flex gap-2">
        <Button onClick={() => select(range)}>{t("Refresh")}</Button>
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

          {stats.byConversation?.length ? (
            <Card className="mt-4">
              <div className="mb-2 text-sm font-semibold text-zinc-200">{t("By conversation")}</div>
              <div className="divide-y divide-white/5">
                {stats.byConversation.map((c) => (
                  <div key={c.conversationId} className="py-1.5 text-sm">
                    <div className="flex items-center justify-between gap-3">
                      <span className="truncate text-zinc-200">
                        {shortConversationId(c.conversationId)}
                      </span>
                      <span className="text-zinc-500">
                        {c.requests} {t("requests")}
                      </span>
                    </div>
                    <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-zinc-400">
                      <span>{t("Input")} {formatTokenDimension(c.inputTokens)}</span>
                      <span>{t("Output")} {formatTokenDimension(c.outputTokens)}</span>
                      <span>{t("Cached")} {formatTokenDimension(c.cachedTokens)}</span>
                      <span>{t("Reasoning")} {formatTokenDimension(c.reasoningTokens)}</span>
                      <span>{t("Rate")} {c.cacheRate ?? "-"}</span>
                      <span>{t("Total")} {formatTokenDimension(c.inputTokens + c.outputTokens)}</span>
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          ) : null}

          {stats.recentRequests?.length ? (
            <Card className="mt-4">
              <div className="mb-2 text-sm font-semibold text-zinc-200">{t("Request details")}</div>
              <div className="overflow-x-auto">
                <table aria-label={t("Request details")} className="w-full text-sm">
                  <thead className="text-left text-xs text-zinc-500">
                    <tr>
                      <th className="pb-1 pr-2">{t("Time")}</th>
                      <th className="pb-1 pr-2">{t("Provider")}</th>
                      <th className="pb-1 pr-2">{t("Model")}</th>
                      <th className="pb-1 pr-2">{t("Status")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Input")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Output")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Cached")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Reasoning")}</th>
                      <th className="pb-1 pr-2 text-right">{t("Rate")}</th>
                      <th className="pb-1 text-right">{t("Total")}</th>
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

function formatRequestStatus(r: RecentRequest): string {
  if (r.ok) {
    return r.status != null ? String(r.status) : "ok";
  }
  const parts = [r.status != null ? String(r.status) : null, r.error ?? null].filter(Boolean);
  return parts.join(" ") || "failed";
}
