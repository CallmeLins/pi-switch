import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { add as addProfile, doctor, installProxyProvider, list, preset, remove as removeProfile, setProxyTarget, update as updateProfile, use } from "../src/commands.js";
import { loadConfig, profileToPiProvider, providerIdFor, saveConfig } from "../src/core.js";
import { getCircuitState, resetCircuitState, daemonStart, daemonStop, daemonStatus } from "../src/proxy.js";
import { getStats, formatStatsText } from "../src/stats.js";
import { exportConfig, importConfig, getProfileInfo, openProvider } from "../src/sync.js";

async function registerConfiguredProviders(pi: ExtensionAPI): Promise<string[]> {
  const config = await loadConfig();
  const registered: string[] = [];

  for (const [name, profile] of Object.entries(config.profiles || {})) {
    const providerId = providerIdFor(config, name);
    pi.registerProvider(providerId, profileToPiProvider(profile as any) as any);
    registered.push(providerId);
  }

  return registered;
}

function formatProfileList(result: Awaited<ReturnType<typeof list>>): string {
  const names = Object.keys(result.profiles);
  if (names.length === 0) return "No profiles. Add one with /piswitch add.";
  return names
    .map((name) => {
      const p = result.profiles[name];
      const mark = result.current === name ? "*" : " ";
      const models = (p.models || []).map((m: any) => m.id).join(", ");
      return `${mark} ${name}\n    api: ${p.api}\n    baseUrl: ${p.baseUrl}\n    models: ${models}`;
    })
    .join("\n");
}

async function showDoctor(ctx: ExtensionContext): Promise<void> {
  const checks = await doctor();
  const text = checks.map((c) => `${c.ok ? "✓" : "✗"} ${c.msg}`).join("\n");
  ctx.ui.notify(text, checks.some((c) => !c.ok) ? "warning" : "info");
}

async function chooseProfile(ctx: ExtensionContext): Promise<string | null> {
  const result = await list();
  const names = Object.keys(result.profiles);
  if (names.length === 0) {
    ctx.ui.notify("No profiles. Add one with /piswitch add.", "warning");
    return null;
  }
  const selected = await ctx.ui.select(
    "pi-switch profiles",
    names.map((name) => {
      const p = result.profiles[name];
      const mark = result.current === name ? "* " : "  ";
      return `${mark}${name}  (${p.api})`;
    }),
  );
  if (!selected) return null;
  return selected.replace(/^\*?\s*/, "").split(/\s+/)[0] || null;
}

async function promptAddProvider(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
  const presetResult = await preset(["list"]);
  const presetItems = presetResult.presets.map((p: any) => `${p.id} — ${p.description}`);
  const selected = await ctx.ui.select("Choose provider preset", [
    ...presetItems,
    "custom-openai — Custom OpenAI-compatible endpoint",
    "custom-anthropic — Custom Anthropic-compatible endpoint",
  ]);
  if (!selected) return;

  const presetId = selected.split(" — ")[0];
  const isCustomOpenAI = presetId === "custom-openai";
  const isCustomAnthropic = presetId === "custom-anthropic";
  const isCustom = isCustomOpenAI || isCustomAnthropic;

  const defaultName = isCustom ? (isCustomOpenAI ? "custom-openai" : "custom-anthropic") : presetId;
  const name = (await ctx.ui.input("Provider name", defaultName))?.trim();
  if (!name) return;

  let baseUrl: string | undefined;
  let api: string | undefined;
  let model: string | undefined;

  if (isCustom) {
    api = isCustomOpenAI ? "openai" : "anthropic";
    const defaultUrl = isCustomOpenAI ? "https://api.example.com/v1" : "https://api.example.com";
    baseUrl = (await ctx.ui.input("Base URL", defaultUrl))?.trim();
    if (!baseUrl) return;
    const defaultModel = isCustomOpenAI ? "model-id" : "claude-sonnet-4-5";
    model = (await ctx.ui.input("Model id", defaultModel))?.trim();
    if (!model) return;
  }

  const defaultKey = isCustom ? "$MY_API_KEY" : `$${presetId.toUpperCase().replace(/-/g, "_")}_API_KEY`;
  const apiKey = (await ctx.ui.input("API key or env var", defaultKey))?.trim();
  if (!apiKey) return;

  const summary = isCustom
    ? `Name: ${name}\nAPI: ${api}\nBase URL: ${baseUrl}\nAPI Key: ${apiKey}\nModel: ${model}`
    : `Name: ${name}\nPreset: ${presetId}\nAPI Key: ${apiKey}`;
  const ok = await ctx.ui.confirm("Create pi-switch provider?", summary);
  if (!ok) return;

  const argv = isCustom
    ? [name, "--api", api!, "--base-url", baseUrl!, "--api-key", apiKey, "--model", model!]
    : [name, "--preset", presetId, "--api-key", apiKey];

  const result = await addProfile(argv);
  const registered = await registerConfiguredProviders(pi);
  const activate = await ctx.ui.confirm(
    "Provider created",
    `Created '${result.name}'.\nRegistered ${registered.length} provider(s).\n\nActivate it now?`,
  );
  if (activate) {
    const activated = await use([result.name]);
    await registerConfiguredProviders(pi);
    ctx.ui.notify(`Activated '${activated.name}' as '${activated.providerId}'. Open /model to select it.`, "info");
  } else {
    ctx.ui.notify(`Created '${result.name}'. Use /piswitch use ${result.name} when ready.`, "info");
  }
}

async function promptEditProvider(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
  const name = await chooseProfile(ctx);
  if (!name) return;
  const result = await list();
  const profile = result.profiles[name];
  if (!profile) return;

  const action = await ctx.ui.select(`Edit ${name}`, [
    "Edit API key",
    "Edit Base URL",
    "Edit models",
    "Delete profile",
  ]);
  if (!action) return;

  if (action === "Edit API key") {
    const apiKey = (await ctx.ui.input("API key or env var", profile.apiKey || "$API_KEY"))?.trim();
    if (!apiKey) return;
    await updateProfile(name, { apiKey });
    await registerConfiguredProviders(pi);
    ctx.ui.notify(`Updated API key for '${name}'.`, "info");
    return;
  }

  if (action === "Edit Base URL") {
    const baseUrl = (await ctx.ui.input("Base URL", profile.baseUrl || "https://api.example.com/v1"))?.trim();
    if (!baseUrl) return;
    await updateProfile(name, { baseUrl });
    await registerConfiguredProviders(pi);
    ctx.ui.notify(`Updated Base URL for '${name}'.`, "info");
    return;
  }

  if (action === "Edit models") {
    const current = (profile.models || []).map((m: any) => (m.name ? `${m.id}=${m.name}` : m.id)).join(", ");
    const text = (await ctx.ui.input("Models, comma separated. Use id or id=name", current))?.trim();
    if (!text) return;
    const models = text.split(",").map((s) => s.trim()).filter(Boolean).map((item) => {
      const idx = item.indexOf("=");
      const id = idx === -1 ? item : item.slice(0, idx).trim();
      const modelName = idx === -1 ? undefined : item.slice(idx + 1).trim();
      return {
        id,
        ...(modelName ? { name: modelName } : {}),
        input: ["text"],
        contextWindow: 128000,
        maxTokens: 16384,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      };
    });
    await updateProfile(name, { models });
    await registerConfiguredProviders(pi);
    ctx.ui.notify(`Updated models for '${name}'.`, "info");
    return;
  }

  if (action === "Delete profile") {
    const ok = await ctx.ui.confirm("Delete provider?", `Delete '${name}' from pi-switch config?`);
    if (!ok) return;
    await removeProfile(name);
    await registerConfiguredProviders(pi);
    ctx.ui.notify(`Deleted '${name}'.`, "info");
  }
}

async function promptShowProvider(ctx: ExtensionContext): Promise<void> {
  const name = await chooseProfile(ctx);
  if (!name) return;
  const result = await list();
  const profile = result.profiles[name];
  if (!profile) return;
  const models = (profile.models || []).map((m: any) => `- ${m.id}${m.name ? ` (${m.name})` : ""}`).join("\n");
  ctx.ui.notify(
    `Name: ${name}\nAPI: ${profile.api}\nBase URL: ${profile.baseUrl}\nAPI Key: ${profile.apiKey}\nPreset: ${profile.preset || "custom"}\nModels:\n${models || "- none"}`,
    "info",
  );
}

async function promptRemoveProvider(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
  const name = await chooseProfile(ctx);
  if (!name) return;
  const ok = await ctx.ui.confirm("Delete provider?", `Delete '${name}' from pi-switch config?`);
  if (!ok) return;
  await removeProfile(name);
  await registerConfiguredProviders(pi);
  ctx.ui.notify(`Deleted '${name}'.`, "info");
}

async function promptRawConfig(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
  const config = await loadConfig();
  const edited = await ctx.ui.editor("Edit ~/.pi-switch/config.json", JSON.stringify(config, null, 2));
  if (edited === undefined) return;
  let parsed: any;
  try {
    parsed = JSON.parse(edited);
  } catch (err: any) {
    ctx.ui.notify(`Invalid JSON: ${err.message}`, "error");
    return;
  }
  const ok = await ctx.ui.confirm("Save raw config?", "This will overwrite ~/.pi-switch/config.json.");
  if (!ok) return;
  await saveConfig(parsed);
  const registered = await registerConfiguredProviders(pi);
  ctx.ui.notify(`Saved raw config. Registered ${registered.length} provider(s).`, "info");
}

async function promptInstallProxy(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
  const action = await ctx.ui.select("Proxy", ["Install proxy provider", "Set target profile", "Circuit status", "Reset circuit", "Daemon start", "Daemon stop", "Daemon status"]);
  if (!action) return;

  if (action === "Daemon start") {
    try {
      const result = await daemonStart({});
      ctx.ui.notify(result.message + (result.pid ? `\nPID: ${result.pid}\nLog: ${result.logPath}` : ""), "info");
    } catch (err: any) {
      ctx.ui.notify(`Daemon start failed: ${err.message}`, "error");
    }
    return;
  }

  if (action === "Daemon stop") {
    const result = await daemonStop();
    ctx.ui.notify(result.message, "info");
    return;
  }

  if (action === "Daemon status") {
    const result = await daemonStatus();
    if (result.running) {
      ctx.ui.notify(`Proxy daemon running\nPID: ${result.pid}\nListen: http://${result.host}:${result.port}\nTarget: ${result.target || "none"}\nFailover: ${result.failover?.join(" -> ") || "none"}`, "info");
    } else {
      ctx.ui.notify(result.message, "warning");
    }
    return;
  }

  if (action === "Set target profile") {
    const target = await chooseNonProxyProfile(ctx);
    if (!target) return;
    const result = await list();
    const others = Object.keys(result.profiles).filter((name) => name !== target && !result.profiles[name]?.proxy);
    const failoverText = others.length
      ? await ctx.ui.input("Failover profiles, comma separated (optional)", others.join(","))
      : undefined;
    const failover = failoverText?.split(",").map((s) => s.trim()).filter(Boolean);
    const saved = await setProxyTarget(target, failover);
    ctx.ui.notify(
      `Proxy target set to '${saved.target}'.${saved.failover.length ? `\nFailover: ${saved.failover.join(" -> ")}` : ""}`,
      "info",
    );
    return;
  }

  if (action === "Circuit status") {
    ctx.ui.notify(JSON.stringify(await getCircuitState(), null, 2), "info");
    return;
  }

  if (action === "Reset circuit") {
    await resetCircuitState();
    ctx.ui.notify("Circuit breaker state reset.", "info");
    return;
  }

  const name = (await ctx.ui.input("Proxy provider name", "proxy"))?.trim();
  if (!name) return;
  const host = (await ctx.ui.input("Proxy host", "127.0.0.1"))?.trim();
  if (!host) return;
  const port = (await ctx.ui.input("Proxy port", "43112"))?.trim();
  if (!port) return;
  const result = await installProxyProvider([name, "--host", host, "--port", port]);
  await registerConfiguredProviders(pi);
  const activate = await ctx.ui.confirm(
    "Proxy provider installed",
    `Installed '${result.name}' -> http://${host}:${port}/v1.\n\nActivate it now?`,
  );
  if (activate) {
    const activated = await use([result.name]);
    ctx.ui.notify(`Activated '${activated.name}'. Start the proxy with: pi-switch proxy start`, "info");
  } else {
    ctx.ui.notify(`Installed '${result.name}'. Start proxy with: pi-switch proxy start`, "info");
  }
}

async function chooseNonProxyProfile(ctx: ExtensionContext): Promise<string | null> {
  const result = await list();
  const names = Object.keys(result.profiles).filter((name) => !result.profiles[name]?.proxy);
  if (names.length === 0) {
    ctx.ui.notify("No non-proxy profiles. Add an upstream provider first with /piswitch add.", "warning");
    return null;
  }
  const selected = await ctx.ui.select(
    "Proxy target profile",
    names.map((name) => `${name}  (${result.profiles[name].api})`),
  );
  return selected ? selected.split(/\s+/)[0] || null : null;
}

export default async function piSwitchExtension(pi: ExtensionAPI) {
  try {
    await registerConfiguredProviders(pi);
  } catch {
    // Avoid breaking pi startup if pi-switch has not been initialized yet.
  }

  pi.registerCommand("piswitch", {
    description: "Manage pi-switch profiles",
    getArgumentCompletions: (prefix) => {
      const commands = ["add", "edit", "show", "remove", "raw", "proxy", "list", "use", "doctor", "reload", "presets", "stats", "info", "open", "export", "import", "menu"];
      const matches = commands.filter((c) => c.startsWith(prefix.trim()));
      return matches.length ? matches.map((value) => ({ value, label: value })) : null;
    },
    handler: async (args, ctx) => {
      const [subcommand, ...rest] = args.trim().split(/\s+/).filter(Boolean);
      const cmd = subcommand || "menu";

      if (cmd === "add") {
        await promptAddProvider(pi, ctx);
        return;
      }

      if (cmd === "edit") {
        await promptEditProvider(pi, ctx);
        return;
      }

      if (cmd === "show") {
        await promptShowProvider(ctx);
        return;
      }

      if (cmd === "remove" || cmd === "rm") {
        await promptRemoveProvider(pi, ctx);
        return;
      }

      if (cmd === "raw") {
        await promptRawConfig(pi, ctx);
        return;
      }

      if (cmd === "proxy") {
        await promptInstallProxy(pi, ctx);
        return;
      }

      if (cmd === "list") {
        ctx.ui.notify(formatProfileList(await list()), "info");
        return;
      }

      if (cmd === "doctor") {
        await showDoctor(ctx);
        return;
      }

      if (cmd === "presets" || cmd === "preset") {
        const result = await preset(["list"]);
        ctx.ui.notify(
          result.presets.map((p: any) => `${p.id}: ${p.description}\n  ${p.baseUrl}`).join("\n"),
          "info",
        );
        return;
      }

      if (cmd === "reload") {
        const registered = await registerConfiguredProviders(pi);
        ctx.ui.notify(`Registered ${registered.length} provider(s): ${registered.join(", ") || "none"}`, "info");
        return;
      }

      if (cmd === "stats") {
        const format = rest[0] || "summary";
        const valid = ["summary", "by-provider", "by-model", "hourly", "full"];
        if (!valid.includes(format)) {
          ctx.ui.notify(`Invalid format '${format}'. Use: ${valid.join("|")}`, "warning");
          return;
        }
        const stats = await getStats();
        ctx.ui.notify(formatStatsText(stats, format), "info");
        return;
      }

      if (cmd === "info") {
        const name = rest[0] || (await chooseProfile(ctx));
        if (!name) return;
        try {
          const result = await getProfileInfo(name);
          const links = [];
          if (result.links.manageKeys) links.push(`Manage keys: ${result.links.manageKeys}`);
          links.push(`API docs: ${result.links.docs}`);
          ctx.ui.notify(
            `Name: ${result.name}\nAPI: ${result.api}\nBase URL: ${result.baseUrl}\nModels: ${result.models.join(", ")}\n\n${links.join("\n")}`,
            "info",
          );
        } catch (err: any) {
          ctx.ui.notify(err.message, "error");
        }
        return;
      }

      if (cmd === "open") {
        const target = rest[0] || (await chooseProfile(ctx));
        if (!target) return;
        const result = await openProvider(target);
        if (result?.opened) {
          ctx.ui.notify(`Opened ${result.label}`, "info");
        } else if (result?.url) {
          ctx.ui.notify(`${result.label}: ${result.url}`, "info");
        } else {
          ctx.ui.notify(`No link for '${target}'`, "warning");
        }
        return;
      }

      if (cmd === "export") {
        const passphrase = await ctx.ui.input("Passphrase (min 8 chars)", "");
        if (!passphrase) return;
        try {
          const result = await exportConfig(passphrase.trim());
          ctx.ui.notify(result.message, "info");
        } catch (err: any) {
          ctx.ui.notify(`Export failed: ${err.message}`, "error");
        }
        return;
      }

      if (cmd === "import") {
        const filePath = await ctx.ui.input("Export file path", "");
        if (!filePath) return;
        const passphrase = await ctx.ui.input("Passphrase", "");
        if (!passphrase) return;
        try {
          const result = await importConfig(filePath.trim(), passphrase.trim());
          ctx.ui.notify(result.message + (result.sanitizedKeys ? `\n${result.sanitizedKeys} key(s) sanitized → env vars` : ""), "info");
          await registerConfiguredProviders(pi);
        } catch (err: any) {
          ctx.ui.notify(`Import failed: ${err.message}`, "error");
        }
        return;
      }

      if (cmd === "use") {
        const name = rest[0] || (await chooseProfile(ctx));
        if (!name) return;
        const result = await use([name]);
        await registerConfiguredProviders(pi);
        ctx.ui.notify(
          `Activated '${result.name}' as '${result.providerId}'. Open /model to refresh/select the model.`,
          "info",
        );
        return;
      }

      if (cmd !== "menu") {
        ctx.ui.notify("Usage: /piswitch [add|edit|show|remove|raw|list|use|doctor|presets|reload|menu]", "warning");
        return;
      }

      const selected = await ctx.ui.select("pi-switch", [
        "Add provider",
        "Edit provider",
        "Show provider",
        "Remove provider",
        "Raw config editor",
        "Install proxy provider",
        "Use profile",
        "List profiles",
        "Preset list",
        "Doctor",
        "Reload registered providers",
        "Usage stats",
        "Open dashboard",
        "Export config",
        "Import config",
      ]);
      if (selected === "Add provider") {
        await promptAddProvider(pi, ctx);
      } else if (selected === "Edit provider") {
        await promptEditProvider(pi, ctx);
      } else if (selected === "Show provider") {
        await promptShowProvider(ctx);
      } else if (selected === "Remove provider") {
        await promptRemoveProvider(pi, ctx);
      } else if (selected === "Raw config editor") {
        await promptRawConfig(pi, ctx);
      } else if (selected === "Install proxy provider") {
        await promptInstallProxy(pi, ctx);
      } else if (selected === "Use profile") {
        const name = await chooseProfile(ctx);
        if (!name) return;
        const result = await use([name]);
        await registerConfiguredProviders(pi);
        ctx.ui.notify(`Activated '${result.name}' as '${result.providerId}'. Open /model to refresh/select it.`, "info");
      } else if (selected === "List profiles") {
        ctx.ui.notify(formatProfileList(await list()), "info");
      } else if (selected === "Preset list") {
        const result = await preset(["list"]);
        ctx.ui.notify(result.presets.map((p: any) => `${p.id}: ${p.description}\n  ${p.baseUrl}`).join("\n"), "info");
      } else if (selected === "Doctor") {
        await showDoctor(ctx);
      } else if (selected === "Reload registered providers") {
        const registered = await registerConfiguredProviders(pi);
        ctx.ui.notify(`Registered ${registered.length} provider(s): ${registered.join(", ") || "none"}`, "info");
      } else if (selected === "Usage stats") {
        const stats = await getStats();
        ctx.ui.notify(formatStatsText(stats, "summary"), "info");
      } else if (selected === "Open dashboard") {
        const name = await chooseProfile(ctx);
        if (!name) return;
        const result = await openProvider(name);
        if (result?.opened) ctx.ui.notify(`Opened ${result.label}`, "info");
        else if (result?.url) ctx.ui.notify(`${result.label}: ${result.url}`, "info");
        else ctx.ui.notify(`No link for '${name}'`, "warning");
      } else if (selected === "Export config") {
        const passphrase = await ctx.ui.input("Passphrase (min 8 chars)", "");
        if (!passphrase) return;
        try {
          const result = await exportConfig(passphrase.trim());
          ctx.ui.notify(result.message, "info");
        } catch (err: any) {
          ctx.ui.notify(`Export failed: ${err.message}`, "error");
        }
      } else if (selected === "Import config") {
        const filePath = await ctx.ui.input("Export file path", "");
        if (!filePath) return;
        const passphrase = await ctx.ui.input("Passphrase", "");
        if (!passphrase) return;
        try {
          const result = await importConfig(filePath.trim(), passphrase.trim());
          ctx.ui.notify(result.message + (result.sanitizedKeys ? `\n${result.sanitizedKeys} key(s) sanitized` : ""), "info");
          await registerConfiguredProviders(pi);
        } catch (err: any) {
          ctx.ui.notify(`Import failed: ${err.message}`, "error");
        }
      }
    },
  });
}
