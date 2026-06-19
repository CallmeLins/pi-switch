#!/usr/bin/env node
import {
  initConfig, addProvider, listProfiles, showProfile, removeProfile,
  listPresets, showPreset, listBackups, doctor,
  daemonStartNative, daemonStopNative, daemonStatusNative,
  getUsageStats, exportConfig, importConfig,
  validateConfig, testProvider, restoreBackup, duplicateProvider,
  exportLogsJson, exportLogsCsv, fetchModels,
  runProxyServer,
  runNativeTui,
  updateExposedModels,
  updateProviderModels,
  setProxyFailover,
} from "../index.js";
import * as readline from "readline";

function usage() {
  console.log(`pi-switch v0.3.6 — lightweight profile switcher for pi agent

Usage:
  pi-switch provider list
  pi-switch provider show <name>
  pi-switch provider add <name> [--preset <preset>] [--api <api>] [--base-url <url>] [--api-key <key>] [--model <id>...]
  pi-switch provider edit <name>
  pi-switch provider delete <name>
  pi-switch provider duplicate <name> [--as <new-name>]
  pi-switch provider test <name>
  pi-switch provider fetch-models <name>
  pi-switch provider models <name> <model-id>...       # Update provider's model list
  pi-switch provider expose <name> <model-id>...       # Expose models to pi agent
  pi-switch presets [list]
  pi-switch presets show <id>
  pi-switch config show
  pi-switch config path
  pi-switch config validate
  pi-switch config export <passphrase>
  pi-switch config import <path> <passphrase>
  pi-switch config backups
  pi-switch config restore <backup-path>
  pi-switch proxy start  [--host <ip>] [--port <port>] [--daemon]
  pi-switch proxy stop
  pi-switch proxy status
  pi-switch proxy failover <profile1,profile2,...>     # Same-model fallback order
  pi-switch stats
  pi-switch logs export [--format json|csv] [--output <file>]
  pi-switch doctor
  pi-switch tui

Gateway Workflow:
  1. Add profiles:          pi-switch provider add <name> ...
  2. Expose models:         choose models per profile (tui: 'x'), or expose all
  3. (optional) Failover:   pi-switch proxy failover <backup1,backup2>
  4. Start proxy:           pi-switch proxy start --daemon
  5. Use in pi:             select the 'pi-switch' provider, then a 'profile/model'
     The proxy routes by the model name in each request — no target to set.

Aliases: remove → provider delete, rm → provider delete, interactive/ui → tui
`);
}

function fail(message, code = 1) {
  const clean = message.startsWith("Error: ") ? message : `Error: ${message}`;
  console.error(clean);
  process.exit(code);
}

function parseModel(modelArg) {
  const idx = modelArg.indexOf("=");
  const id = idx === -1 ? modelArg.trim() : modelArg.slice(0, idx).trim();
  const name = idx === -1 ? undefined : modelArg.slice(idx + 1).trim();
  if (!id) throw new Error(`invalid --model '${modelArg}'`);
  return { id, ...(name ? { name } : {}), input: ["text"], contextWindow: 128000, maxTokens: 16384, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } };
}

function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg.startsWith("--")) { out._.push(arg); continue; }
    const eq = arg.indexOf("=");
    const key = eq === -1 ? arg.slice(2) : arg.slice(2, eq);
    const raw = eq === -1 ? argv[++i] : arg.slice(eq + 1);
    if (raw === undefined || raw.startsWith("--")) throw new Error(`missing value for --${key}`);
    if (out[key] === undefined) out[key] = raw;
    else if (Array.isArray(out[key])) out[key].push(raw);
    else out[key] = [out[key], raw];
  }
  return out;
}

function asArray(value) {
  if (value === undefined) return [];
  return Array.isArray(value) ? value : [value];
}

async function editProviderInteractive(name) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  const prompt = (question, defaultValue) => {
    return new Promise((resolve) => {
      const display = defaultValue !== undefined && defaultValue !== ""
        ? `${question} [${defaultValue}]: `
        : `${question}: `;
      rl.question(display, (answer) => {
        resolve(answer.trim() || defaultValue || "");
      });
    });
  };

  try {
    // Load current profile
    const data = JSON.parse(showProfile(name));
    const profile = data.profile;

    console.log(`\nEditing provider '${name}'`);
    console.log("─".repeat(60));
    console.log(`Current API: ${profile.api}`);
    console.log(`Current Base URL: ${profile.baseUrl}`);
    console.log(`Current API Key: ${profile.apiKey.slice(0, 8)}...`);
    console.log(`Current Models: ${profile.models?.map(m => m.id).join(", ") || "none"}`);
    console.log("─".repeat(60));
    console.log("\nPress Enter to keep current value, or type new value:\n");

    // Prompt for each field
    const newName = await prompt("Provider name", name);

    let newApi = await prompt("API type (openai-completions | anthropic-messages | google-generative-ai)", profile.api);
    // Normalize aliases
    if (newApi === "openai") newApi = "openai-completions";
    if (newApi === "anthropic") newApi = "anthropic-messages";

    const newBaseUrl = await prompt("Base URL", profile.baseUrl);
    const newApiKey = await prompt("API Key", profile.apiKey);

    const currentModels = profile.models?.map(m => m.id).join(",") || "";
    const newModelsStr = await prompt("Models (comma-separated)", currentModels);

    // Build updated profile
    const updatedProfile = {
      api: newApi,
      baseUrl: newBaseUrl,
      apiKey: newApiKey,
      models: newModelsStr.split(",").filter(s => s.trim()).map(id => ({
        id: id.trim(),
        input: ["text"],
        contextWindow: 128000,
        maxTokens: 16384,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }
      })),
      preset: profile.preset,
      headers: profile.headers,
      authHeader: profile.authHeader,
      compat: profile.compat,
      proxy: profile.proxy || false,
      updatedAt: new Date().toISOString(),
    };

    // Show summary
    console.log("\n" + "─".repeat(60));
    console.log("Updated configuration:");
    console.log("─".repeat(60));
    console.log(`Name: ${newName}`);
    console.log(`API: ${updatedProfile.api}`);
    console.log(`Base URL: ${updatedProfile.baseUrl}`);
    console.log(`API Key: ${updatedProfile.apiKey.slice(0, 8)}...`);
    console.log(`Models: ${updatedProfile.models.map(m => m.id).join(", ")}`);
    console.log("─".repeat(60));

    const confirm = await prompt("\nSave changes? (y/n)", "y");
    if (confirm.toLowerCase() !== "y" && confirm.toLowerCase() !== "yes") {
      console.log("Edit cancelled.");
      return;
    }

    // Save via native binding (upsert_profile with rename_from)
    const { upsertProfileRaw } = await import("../index.js");
    const renameFrom = newName !== name ? name : undefined;
    const result = upsertProfileRaw(newName, JSON.stringify(updatedProfile), renameFrom);

    console.log(`\n✓ Provider '${newName}' updated successfully`);
    if (result.backup) {
      console.log(`Backup: ${result.backup}`);
    }
  } finally {
    rl.close();
  }
}

async function main() {
  const [cmd, ...rest] = process.argv.slice(2);
  try {
    if (!cmd || cmd === "help" || cmd === "--help" || cmd === "-h") return usage();

    // ─── Aliases ─────────────────────────────────────

    const effectiveCmd = (() => {
      if (cmd === "remove" || cmd === "rm") return "provider-delete";
      if (cmd === "interactive" || cmd === "ui") return "tui";
      return cmd;
    })();

    // ─── Init ────────────────────────────────────────

    if (effectiveCmd === "init") {
      for (const line of initConfig()) console.log(line);
      return;
    }

    // ─── Provider subcommands ────────────────────────

    if (effectiveCmd === "provider" || effectiveCmd === "provider-delete") {
      const sub = effectiveCmd === "provider-delete" ? "delete" : (rest[0] || "list");

      if (sub === "list") {
        const data = JSON.parse(listProfiles());
        const names = Object.keys(data.profiles);
        if (names.length === 0) { console.log("No profiles. Add one with: pi-switch provider add <name> ..."); return; }

        // Get proxy configuration
        const target = data.settings?.proxy?.target;
        const failoverChain = data.settings?.proxy?.failover || [];

        // Build failover priority map (target is p0, failover are p1, p2, ...)
        const priorityMap = new Map();
        if (target) priorityMap.set(target, 0);
        failoverChain.forEach((name, idx) => priorityMap.set(name, idx + 1));

        // Get circuit breaker status
        const stats = JSON.parse(getUsageStats());
        const circuitBreakerStatus = stats.circuitBreaker || {};

        // ANSI colors
        const RED = '\x1b[31m';
        const GREEN = '\x1b[32m';
        const DIM = '\x1b[2m';
        const RESET = '\x1b[0m';

        for (const name of names) {
          const p = data.profiles[name];
          const models = (p.models || []).map(m => m.id).join(", ");

          // Check if in failover chain
          const priority = priorityMap.get(name);
          const inChain = priority !== undefined;
          const mark = inChain ? "*" : " ";

          // Check circuit breaker status
          const cbStatus = circuitBreakerStatus[name];
          const isOpen = cbStatus?.state === "open" || cbStatus?.state === "half_open";

          // Build priority label
          let priorityLabel = "";
          if (inChain) {
            priorityLabel = priority === 0 ? " [target]" : ` [p${priority}]`;
          }

          // Color based on circuit breaker state
          let color = "";
          if (inChain) {
            color = isOpen ? RED : GREEN;
          }

          // First line: name + priority
          console.log(`${color}${mark} ${name}${priorityLabel}${RESET}`);

          // Second line: api + baseUrl (dimmed if in chain)
          const lineColor = inChain ? (isOpen ? RED + DIM : GREEN + DIM) : "";
          console.log(`${lineColor}    api: ${p.api}    baseUrl: ${p.baseUrl}${lineColor ? RESET : ""}`);

          // Third line: models (if any)
          if (models) {
            console.log(`${lineColor}    models: ${models}${lineColor ? RESET : ""}`);
          }
        }
        return;
      }

      if (sub === "show" || sub === "info") {
        const name = rest[1];
        if (!name) fail("provider name required");
        const result = JSON.parse(showProfile(name));
        console.log(JSON.stringify(result, null, 2));
        return;
      }

      if (sub === "add") {
        const args = parseArgs(rest.slice(1));
        const name = args._[0];
        if (!name) fail("provider name required");
        const modelArgs = asArray(args.model);
        const result = addProvider({
          name,
          preset: args.preset || undefined,
          api: args.api || undefined,
          baseUrl: args["base-url"] || args.baseUrl || undefined,
          apiKey: args["api-key"] || args.apiKey || undefined,
          models: modelArgs.length ? modelArgs.map(m => typeof m === 'string' ? parseModel(m).id : m) : undefined,
        });
        console.log(`Saved profile '${result.name}' to ~/.pi-switch/config.json`);
        if (result.backup) console.log(`Backup: ${result.backup}`);
        return;
      }

      if (sub === "edit") {
        const name = rest[1];
        if (!name) fail("provider name required");
        await editProviderInteractive(name);
        return;
      }

      if (sub === "delete" || sub === "remove" || sub === "rm") {
        const name = rest[1];
        if (!name) fail("provider name required");
        const result = removeProfile(name);
        console.log(`Removed profile '${result.name}'`);
        if (result.backup) console.log(`Backup: ${result.backup}`);
        return;
      }

      if (sub === "duplicate" || sub === "copy") {
        const name = rest[1];
        if (!name) fail("provider name required");
        const args = parseArgs(rest.slice(2));
        const dst = args.as || args._[0] || `${name}-copy`;
        const result = duplicateProvider(name, dst);
        console.log(result);
        return;
      }

      if (sub === "test") {
        const name = rest[1];
        if (!name) fail("provider name required");
        console.log(`Testing provider '${name}'...`);
        const result = await testProvider(name);
        console.log(result.message);
        if (result.responseTimeMs !== undefined && result.responseTimeMs !== null) {
          console.log(`Response time: ${result.responseTimeMs}ms`);
        }
        process.exit(result.success ? 0 : 1);
      }

      if (sub === "fetch-models") {
        const name = rest[1];
        if (!name) fail("provider name required");
        console.log(`Fetching models for provider '${name}'...`);
        try {
          const models = await fetchModels(name);
          console.log(`\nFound ${models.length} model(s):\n`);
          for (const model of models) {
            console.log(`  ${model}`);
          }
          console.log(`\nTo update provider models: pi-switch provider models ${name} <model-ids...>`);
          console.log(`To expose to pi agent: pi-switch provider expose ${name} <model-ids...>`);
        } catch (err) {
          fail(err.message);
        }
        return;
      }

      if (sub === "models") {
        const name = rest[1];
        if (!name) fail("provider name required");
        const modelIds = rest.slice(2);
        if (modelIds.length === 0) fail("at least one model ID required");

        // Convert model IDs to ModelEntryInput format
        const models = modelIds.map(id => ({
          id,
          name: undefined,
          input: ["text"],
          contextWindow: 1000000,
          maxTokens: 128000,
        }));

        const result = updateProviderModels(name, models);
        console.log(result);
        console.log(`\nUpdated ${modelIds.length} model(s) for provider '${name}'`);
        console.log(`To expose these to pi agent: pi-switch provider expose ${name} ${modelIds.join(' ')}`);
        return;
      }

      if (sub === "expose") {
        const name = rest[1];
        if (!name) fail("provider name required");
        const modelIds = rest.slice(2);
        if (modelIds.length === 0) fail("at least one model ID required");

        const result = updateExposedModels(name, modelIds);
        console.log(result);
        console.log(`\nExposed ${modelIds.length} model(s) to pi agent`);
        console.log(`Restart pi to see changes in /model`);
        return;
      }

      fail(`unknown provider subcommand: '${sub}'`);
    }

    // ─── Presets ─────────────────────────────────────

    if (effectiveCmd === "presets" || effectiveCmd === "preset") {
      const sub = rest[0] || "list";
      if (sub === "list") {
        let first = true;
        for (const p of listPresets()) {
          if (!first) console.log("");
          first = false;
          console.log(`${p.id}  — ${p.description}`);
          console.log(`    api: ${p.api}    baseUrl: ${p.baseUrl}`);
          console.log(`    models: ${p.models.join(", ")}`);
        }
        return;
      }
      if (sub === "show") {
        const id = rest[1];
        if (!id) fail("preset id required");
        console.log(showPreset(id));
        return;
      }
      fail("usage: pi-switch presets [list|show <id>]");
    }

    // ─── Config subcommands ──────────────────────────

    if (effectiveCmd === "config") {
      const sub = rest[0] || "show";
      if (sub === "show" || sub === "list") {
        const data = JSON.parse(listProfiles());
        console.log(JSON.stringify(data, null, 2));
        return;
      }
      if (sub === "path") {
        console.log(JSON.parse(listProfiles())._configPath || "~/.pi-switch/config.json");
        return;
      }
      if (sub === "export") {
        const pw = rest[1];
        if (!pw) fail("passphrase required");
        console.log(exportConfig(pw));
        return;
      }
      if (sub === "import") {
        const filePath = rest[1];
        const passphrase = rest[2];
        if (!filePath || !passphrase) fail("usage: pi-switch config import <path> <passphrase>");
        console.log(importConfig(filePath, passphrase));
        return;
      }
      if (sub === "backups" || sub === "backup") {
        const result = listBackups();
        if (result.length === 0) console.log("No backups found.");
        else for (const file of result) console.log(file);
        return;
      }
      if (sub === "validate") {
        const issues = validateConfig();
        if (issues.length === 0) {
          console.log("✓ Configuration is valid");
          return;
        }
        console.log(`Found ${issues.length} issue(s):\n`);
        for (const issue of issues) {
          const prefix = issue.level === "error" ? "✗" : "⚠";
          console.log(`${prefix} [${issue.level}] ${issue.path}`);
          console.log(`  ${issue.message}\n`);
        }
        process.exit(issues.some(i => i.level === "error") ? 1 : 0);
      }
      if (sub === "restore") {
        const backupPath = rest[1];
        if (!backupPath) fail("backup path required");
        const result = restoreBackup(backupPath);
        console.log(result);
        return;
      }
      fail(`unknown config subcommand: '${sub}'`);
    }

    // ─── Proxy subcommands ───────────────────────────

    if (effectiveCmd === "proxy") {
      const sub = rest[0] || "status";

      if (sub === "start") {
        const args = {};
        for (let i = 1; i < rest.length; i++) {
          if (rest[i] === "--host") args.host = rest[++i];
          else if (rest[i] === "--port") args.port = parseInt(rest[++i], 10);
          else if (rest[i] === "--daemon") args.daemon = true;
        }

        const config = JSON.parse(listProfiles());
        const host = args.host || config.settings?.proxy?.host || "127.0.0.1";
        const port = args.port || config.settings?.proxy?.port || 43112;

        if (args.daemon) {
          // Daemon mode: fork background process
          const result = JSON.parse(daemonStartNative(host, port));
          console.log(result.message);
          if (result.pid) console.log(`PID: ${result.pid}`);
        } else {
          // Foreground mode: run server directly
          console.log(`Starting proxy server on http://${host}:${port} (foreground mode)`);
          console.log(`Press Ctrl+C to stop`);
          await runProxyServer(host, port);
        }
        return;
      }

      if (sub === "stop") {
        const result = JSON.parse(daemonStopNative());
        console.log(result.message);
        return;
      }

      if (sub === "status") {
        const result = JSON.parse(daemonStatusNative());
        if (result.running) {
          console.log(`Proxy daemon is running (PID ${result.pid})`);
          console.log(`Listen: http://${result.host}:${result.port}`);
          if (result.target) console.log(`Target: ${result.target}`);
          if (result.failover?.length) console.log(`Failover: ${result.failover.join(" → ")}`);
        } else {
          console.log(result.message);
        }
        return;
      }

      if (sub === "failover") {
        const profiles = rest[1];
        if (!profiles) fail("failover profiles required (comma-separated)");
        const profileList = profiles.split(',').map(s => s.trim()).filter(Boolean);
        const result = setProxyFailover(profileList);
        console.log(result);
        return;
      }

      fail("usage: pi-switch proxy [start|stop|status|failover]");
    }

    // ─── Stats ───────────────────────────────────────

    if (effectiveCmd === "stats") {
      const stats = JSON.parse(getUsageStats());
      if (stats.totalRequests === 0) {
        console.log("No request data. Start the proxy and make some requests first.");
        return;
      }
      console.log(`Total: ${stats.totalRequests} | OK: ${stats.okRequests} | Fail: ${stats.failedRequests} | Rate: ${stats.successRate}`);
      if (stats.avgLatencyMs) console.log(`Avg latency: ${stats.avgLatencyMs}ms`);
      if (Object.keys(stats.byProvider).length) {
        console.log("\nBy Provider:");
        for (const [name, ps] of Object.entries(stats.byProvider)) {
          const rate = ps.total > 0 ? ((ps.ok / ps.total) * 100).toFixed(0) + "%" : "0%";
          console.log(`  ${name}: ${ps.total} req, ${ps.ok} ok (${rate})`);
        }
      }
      return;
    }

    // ─── Logs export ─────────────────────────────────

    if (effectiveCmd === "logs") {
      const sub = rest[0] || "export";
      if (sub !== "export") {
        fail("usage: pi-switch logs export [--format json|csv] [--output <file>]");
      }

      const args = parseArgs(rest.slice(1));
      const format = args.format || "json";
      const outputFile = args.output || args._[0] || null;

      let content;
      if (format === "csv") {
        content = exportLogsCsv();
      } else if (format === "json") {
        content = exportLogsJson();
      } else {
        fail(`Unknown format '${format}'. Use json or csv.`);
      }

      if (outputFile) {
        const fs = await import('fs');
        fs.writeFileSync(outputFile, content, 'utf-8');
        console.log(`Logs exported to: ${outputFile}`);
      } else {
        console.log(content);
      }
      return;
    }

    // ─── Doctor ──────────────────────────────────────

    if (effectiveCmd === "doctor") {
      const checks = doctor();
      let failed = 0;
      for (const c of checks) {
        console.log(`${c.ok ? "✓" : "✗"} ${c.msg}`);
        if (!c.ok) failed++;
      }
      if (failed) process.exit(1);
      return;
    }

    // ─── TUI ─────────────────────────────────────────

    if (effectiveCmd === "tui") {
      runNativeTui();
      return;
    }

    // ─── Legacy flat commands (backward compat) ──────

    if (cmd === "list") {
      const data = JSON.parse(listProfiles());
      const names = Object.keys(data.profiles);
      if (names.length === 0) { console.log("No profiles. Add one with: pi-switch provider add <name> ..."); return; }
      for (const name of names) {
        const p = data.profiles[name];
        const mark = data.current === name ? "*" : " ";
        const models = (p.models || []).map(m => m.id).join(", ");
        console.log(`${mark} ${name}`);
        console.log(`    api: ${p.api}    baseUrl: ${p.baseUrl}`);
        if (models) console.log(`    models: ${models}`);
      }
      return;
    }

    if (cmd === "add") {
      const args = parseArgs(rest);
      const name = args._[0];
      if (!name) fail("provider name required");
      const modelArgs = asArray(args.model);
      const result = addProvider({
        name,
        preset: args.preset || undefined,
        api: args.api || undefined,
        baseUrl: args["base-url"] || args.baseUrl || undefined,
        apiKey: args["api-key"] || args.apiKey || undefined,
        models: modelArgs.length ? modelArgs.map(m => typeof m === 'string' ? parseModel(m).id : m) : undefined,
      });
      console.log(`Saved profile '${result.name}' to ~/.pi-switch/config.json`);
      if (result.backup) console.log(`Backup: ${result.backup}`);
      return;
    }

    fail(`unknown command '${cmd}'. Run 'pi-switch help' for usage.`);
  } catch (err) {
    fail(err.stack || err.message);
  }
}

main();
