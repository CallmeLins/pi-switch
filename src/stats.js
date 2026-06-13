import { readFile, access } from "node:fs/promises";
import { join } from "node:path";
import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
import { CONFIG_DIR, loadConfig } from "./core.js";

const LOG_PATH = join(CONFIG_DIR, "requests.log");

async function readLogLines() {
  const entries = [];
  try {
    await access(LOG_PATH);
    const rl = createInterface({
      input: createReadStream(LOG_PATH),
      crlfDelay: Infinity,
    });
    for await (const line of rl) {
      if (!line.trim()) continue;
      try {
        entries.push(JSON.parse(line));
      } catch {
        // Skip malformed lines
      }
    }
  } catch {
    // No log file yet
  }
  return entries;
}

export async function getStats(options = {}) {
  const entries = await readLogLines();
  const config = await loadConfig();

  const stats = {
    totalRequests: entries.length,
    okRequests: 0,
    failedRequests: 0,
    retriedRequests: 0,
    skippedByCircuit: 0,
    byProvider: {},
    byModel: {},
    hourlyDistribution: {},
    avgLatencyMs: 0,
    totalLatencyMs: 0,
    timeframe: { start: null, end: null },
    providers: {},
  };

  for (const entry of entries) {
    if (entry.ok === true) stats.okRequests++;
    else stats.failedRequests++;
    if (entry.retry) stats.retriedRequests++;
    if (entry.skipped) stats.skippedByCircuit++;

    // Per provider
    const provider = entry.provider || "unknown";
    stats.byProvider[provider] = stats.byProvider[provider] || {
      total: 0, ok: 0, failed: 0, retries: 0, avgMs: 0, totalMs: 0, lastUsed: null,
    };
    const ps = stats.byProvider[provider];
    ps.total++;
    if (entry.ok === true) ps.ok++;
    else ps.failed++;
    if (entry.retry) ps.retries++;
    if (entry.ms) {
      ps.totalMs += entry.ms;
      ps.avgMs = Math.round(ps.totalMs / ps.total);
    }
    if (entry.ts) {
      ps.lastUsed = entry.ts;
    }

    // Per model
    const model = entry.model || "unknown";
    stats.byModel[model] = stats.byModel[model] || { total: 0, ok: 0 };
    stats.byModel[model].total++;
    if (entry.ok === true) stats.byModel[model].ok++;

    // Latency
    if (entry.ms) {
      stats.totalLatencyMs += entry.ms;
    }

    // Hourly distribution
    if (entry.ts) {
      const hour = entry.ts.slice(0, 13); // "2026-06-12T09"
      stats.hourlyDistribution[hour] = (stats.hourlyDistribution[hour] || 0) + 1;
    }

    // Timeframe
    if (entry.ts) {
      if (!stats.timeframe.start || entry.ts < stats.timeframe.start) stats.timeframe.start = entry.ts;
      if (!stats.timeframe.end || entry.ts > stats.timeframe.end) stats.timeframe.end = entry.ts;
    }
  }

  if (stats.totalRequests > 0) {
    stats.avgLatencyMs = Math.round(stats.totalLatencyMs / (stats.byProvider ? Object.values(stats.byProvider).reduce((s, p) => s + p.total, 0) : stats.totalRequests));
    stats.successRate = stats.totalRequests > 0 ? ((stats.okRequests / stats.totalRequests) * 100).toFixed(1) + "%" : "0%";
  }

  // Add config provider info
  for (const [name, profile] of Object.entries(config.profiles || {})) {
    stats.providers[name] = {
      api: profile.api,
      baseUrl: profile.baseUrl,
      isProxy: Boolean(profile.proxy),
      isCurrent: config.current === name,
      models: (profile.models || []).map((m) => m.id),
    };
  }

  return stats;
}

export function formatStatsText(stats, format = "summary") {
  if (stats.totalRequests === 0) {
    return "No request data. Start the proxy and make some requests first.";
  }

  const lines = [];

  if (format === "summary" || format === "full") {
    lines.push("Overview");
    lines.push("  Total requests: " + stats.totalRequests);
    lines.push("  Success: " + stats.okRequests + " (" + stats.successRate + ")");
    lines.push("  Failed: " + stats.failedRequests);
    lines.push("  Retried: " + stats.retriedRequests);
    lines.push("  Skipped (circuit): " + stats.skippedByCircuit);
    if (stats.avgLatencyMs) lines.push("  Avg latency: " + stats.avgLatencyMs + "ms");
    if (stats.timeframe.start) {
      lines.push("  From: " + stats.timeframe.start);
      lines.push("  To:   " + stats.timeframe.end);
    }
  }

  if (format === "by-provider" || format === "full") {
    lines.push("\nBy Provider");
    const providers = Object.entries(stats.byProvider);
    if (providers.length === 0) {
      lines.push("  No data");
    } else {
      providers.sort((a, b) => b[1].total - a[1].total);
      for (const [name, ps] of providers) {
        const rate = ps.total > 0 ? ((ps.ok / ps.total) * 100).toFixed(0) + "%" : "0%";
        lines.push(`  ${name}`);
        lines.push(`    total: ${ps.total} | ok: ${ps.ok} | fail: ${ps.failed} | rate: ${rate}`);
        if (ps.avgMs) lines.push(`    avg: ${ps.avgMs}ms | retries: ${ps.retries}`);
        if (ps.lastUsed) lines.push(`    last: ${ps.lastUsed}`);
      }
    }
  }

  if (format === "by-model" || format === "full") {
    lines.push("\nBy Model");
    const models = Object.entries(stats.byModel);
    if (models.length === 0) {
      lines.push("  No data");
    } else {
      models.sort((a, b) => b[1].total - a[1].total);
      for (const [model, ms] of models) {
        const rate = ms.total > 0 ? ((ms.ok / ms.total) * 100).toFixed(0) + "%" : "0%";
        lines.push(`  ${model}: ${ms.total} req (${rate})`);
      }
    }
  }

  if (format === "hourly" || format === "full") {
    lines.push("\nHourly Distribution");
    const hours = Object.entries(stats.hourlyDistribution).sort();
    for (const [hour, count] of hours) {
      const bar = "█".repeat(Math.min(count, 40));
      lines.push(`  ${hour}: ${bar} ${count}`);
    }
  }

  return lines.join("\n");
}
