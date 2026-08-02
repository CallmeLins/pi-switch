import { useEffect, useState } from "react";
import type { AppState, UsageStats } from "../types";
import { api, logsExportUrl } from "../api";
import { Button, Card, SectionTitle } from "./ui";
import { formatTokenCount, formatTotalTokens, shortConversationId } from "../lib/format";

export function StatsPanel(_: { state: AppState; refresh: () => Promise<void> }) {
  const [stats, setStats] = useState<UsageStats | null>(null);

  const load = async () => {
    try {
      setStats(await api.stats());
    } catch {
      setStats(null);
    }
  };
  useEffect(() => {
    void load();
  }, []);

  const byProvider = stats?.byProvider ? Object.entries(stats.byProvider) : [];

  return (
    <div>
      <SectionTitle hint="proxy request usage">Stats</SectionTitle>

      <div className="mb-3 flex gap-2">
        <Button onClick={() => void load()}>Refresh</Button>
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
          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6">
            <Metric label="Total" value={stats.totalRequests} />
            <Metric label="OK" value={stats.okRequests} tone="green" />
            <Metric label="Failed" value={stats.failedRequests} tone="red" />
            <Metric label="Success" value={stats.successRate} />
            <Metric label="Tokens" value={formatTotalTokens(stats.totalTokens)} />
            <Metric label="Cache 率" value={stats.cacheHitRate ?? "-"} />
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

          {stats.byConversation?.length ? (
            <Card className="mt-4">
              <div className="mb-2 text-sm font-semibold text-zinc-200">By conversation</div>
              <div className="divide-y divide-white/5">
                {stats.byConversation.map((c) => (
                  <div
                    key={c.conversationId}
                    className="flex items-center justify-between gap-3 py-1.5 text-sm"
                  >
                    <span className="truncate text-zinc-200">
                      {shortConversationId(c.conversationId)}
                    </span>
                    <span className="text-zinc-500">{c.requests} requests</span>
                    <span className="text-zinc-400">
                      {formatTokenCount(c.inputTokens + c.outputTokens)}
                    </span>
                  </div>
                ))}
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
}: {
  label: string;
  value: string | number;
  tone?: "zinc" | "green" | "red";
}) {
  const color =
    tone === "green" ? "text-emerald-300" : tone === "red" ? "text-red-300" : "text-zinc-100";
  return (
    <Card className="py-3">
      <div className="text-[11px] uppercase tracking-wide text-zinc-500">{label}</div>
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
