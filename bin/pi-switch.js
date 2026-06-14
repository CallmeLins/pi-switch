#!/usr/bin/env node
import {
  initConfig, addProvider, listProfiles, showProfile, useProfile, removeProfile,
  listPresets, showPreset, listBackups, doctor,
  daemonStartNative, daemonStopNative, daemonStatusNative,
  getUsageStats, exportConfig, importConfig,
  validateConfig, testProvider, restoreBackup, duplicateProvider,
  exportLogsJson, exportLogsCsv, fetchModels,
  runNativeTui,
} from "../index.js";

function usage() {
  console.log(`pi-switch v0.2.0 — lightweight profile switcher for pi agent

Usage:
  pi-switch provider list
  pi-switch provider show <name>
  pi-switch provider add <name> [--preset <preset>] [--api <api>] [--base-url <url>] [--api-key <key>] [--model <id>...]
  pi-switch provider edit <name>
  pi-switch provider delete <name>
  pi-switch provider duplicate <name> [--as <new-name>]
  pi-switch provider test <name>
  pi-switch provider fetch-models <name>
  pi-switch use <name> [--mode merge|exclusive]
  pi-switch presets [list]
  pi-switch presets show <id>
  pi-switch config show
  pi-switch config path
  pi-switch config validate
  pi-switch config export <passphrase>
  pi-switch config import <path> <passphrase>
  pi-switch config backups
  pi-switch config restore <backup-path>
  pi-switch proxy start  [--host <ip>] [--port <port>] [--profile <name>]
  pi-switch proxy stop
  pi-switch proxy status
  pi-switch stats
  pi-switch logs export [--format json|csv] [--output <file>]
  pi-switch doctor
  pi-switch tui

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
        // provider edit opens TUI at the edit form for the given provider
        const name = rest[1];
        if (!name) fail("provider name required");
        const data = JSON.parse(showProfile(name));
        console.log(`Current config for '${name}':`);
        console.log(JSON.stringify(data, null, 2));
        console.log(`\nTo edit, use: pi-switch tui → Profiles → ${name} → e (edit)`);
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
          console.log(`\nTo use these models, edit the provider and add them to the models field.`);
        } catch (err) {
          fail(err.message);
        }
        return;
      }

      fail(`unknown provider subcommand: '${sub}'`);
    }

    // ─── Use (shortcut) ──────────────────────────────

    if (effectiveCmd === "use") {
      const args = parseArgs(rest);
      const name = args._[0];
      if (!name) fail("profile name required");
      const result = useProfile(name, args.mode || undefined);
      console.log(`Activated '${result.name}' as provider '${result.providerId}'`);
      if (result.modelsBackup) console.log(`Backup: ${result.modelsBackup}`);
      console.log("Open /model in pi to refresh model list if pi is already running.");
      return;
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
          else if (rest[i] === "--profile") args.profile = rest[++i];
        }
        const result = JSON.parse(daemonStartNative(args.host || null, args.port || null));
        console.log(result.message);
        if (result.pid) console.log(`PID: ${result.pid}`);
        if (args.profile) {
          const useResult = useProfile(args.profile);
          console.log(`Using profile '${useResult.name}' as provider '${useResult.providerId}'`);
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

      fail("usage: pi-switch proxy [start|stop|status]");
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
