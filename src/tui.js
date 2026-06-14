import { add, init, remove, update, use, installProxyProvider, setProxyTarget } from "./commands.js";
import { loadConfig } from "./core.js";
import { daemonStart, daemonStop, daemonStatus, getCircuitState, resetCircuitState } from "./proxy.js";
import { listPresets } from "./presets.js";
import { getStats, formatStatsText } from "./stats.js";
import { exportConfig, importConfig, getProfileInfo, openProvider } from "./sync.js";

// ─── ANSI constants ───────────────────────────────────────

const CSI = "\x1b[";
const UP = (n) => `${CSI}${n}A`;
const DOWN = (n) => `${CSI}${n}B`;
const CURSOR_HIDE = `${CSI}?25l`;
const CURSOR_SHOW = `${CSI}?25h`;
const CLEAR_SCREEN = `${CSI}2J${CSI}H`;
const CLEAR_LINE = `${CSI}2K`;

function color(text, code) {
  if (!code) return text;
  return `\x1b[${code}m${text}\x1b[0m`;
}

const C = {
  bold: 1, dim: 2, reverse: 7,
  green: 32, yellow: 33, cyan: 36, red: 31, magenta: 35,
};

function banner() {
  const w = 41;
  const top = "┌" + "─".repeat(w - 2) + "┐";
  const mid = "│" + " ".repeat(12) + "pi-switch TUI" + " ".repeat(12) + "│";
  const bot = "└" + "─".repeat(w - 2) + "┘";
  return color(top + "\n" + mid + "\n" + bot, C.cyan + C.bold);
}

function divider(label) {
  const w = 41;
  const side = Math.max(0, w - label.length - 2) >> 1;
  return color("─".repeat(side) + ` ${label} ` + "─".repeat(side), C.dim);
}

// ─── Raw keyboard input ───────────────────────────────────

let rawActive = false;

function rawOn() {
  if (rawActive) return;
  rawActive = true;
  process.stdin.setRawMode(true);
  process.stdin.resume();
  process.stdout.write(CURSOR_HIDE);
}

function rawOff() {
  if (!rawActive) return;
  rawActive = false;
  process.stdin.setRawMode(false);
  process.stdout.write(CURSOR_SHOW);
}

function keypress() {
  return new Promise((resolve) => {
    rawOn();
    const onData = (buf) => {
      const str = buf.toString("utf8");
      if (str === "\x03") { rawOff(); process.stdout.write("\n"); process.exit(0); }
      resolve(str);
    };
    process.stdin.once("data", onData);
  });
}

async function readKey() {
  const first = await keypress();
  if (first !== "\x1b") return { key: first, raw: first };
  const second = await Promise.race([keypress(), new Promise((r) => setTimeout(() => r(null), 25))]);
  if (second === null) return { key: "escape", raw: "\x1b" };
  if (second === "[") {
    const third = await keypress();
    const map = { A: "up", B: "down", C: "right", D: "left", H: "home", F: "end" };
    return { key: map[third] || `esc[${third}]`, raw: `\x1b[${third}` };
  }
  return { key: "escape", raw: "\x1b" };
}

// ─── Arrow-key interactive menu ───────────────────────────

async function interactiveMenu(items, { title, backLabel = "← Back", showBack = true } = {}) {
  const displayItems = [...items];
  if (showBack) displayItems.push({ label: backLabel, value: null, isBack: true });

  let sel = 0;

  function buildLines() {
    const lines = [];
    if (title) { lines.push(color("  " + title, C.yellow + C.bold)); lines.push(""); }
    for (let i = 0; i < displayItems.length; i++) {
      const item = displayItems[i];
      const isSel = i === sel;
      const marker = isSel ? color("▶", C.green + C.bold) : " ";
      let lbl = item.label;
      if (isSel) lbl = color(lbl, C.reverse);
      if (item.isBack) lbl = color(lbl, C.dim);
      lines.push("  " + marker + " " + lbl);
      if (item.desc) lines.push("       " + color(item.desc, C.dim));
    }
    return lines;
  }

  function render() {
    const lines = buildLines();
    process.stdout.write(UP(lines.length));
    for (const line of lines) process.stdout.write(CLEAR_LINE + line + "\n");
  }

  // Initial draw
  process.stdout.write("\n");
  const firstLines = buildLines();
  const menuH = firstLines.length;
  for (const line of firstLines) process.stdout.write(CLEAR_LINE + line + "\n");
  process.stdout.write(UP(menuH));

  while (true) {
    const { key } = await readKey();
    switch (key) {
      case "up": case "k":      sel = (sel - 1 + displayItems.length) % displayItems.length; break;
      case "down": case "j":    sel = (sel + 1) % displayItems.length; break;
      case "home":              sel = 0; break;
      case "end":               sel = displayItems.length - 1; break;
      case "\r": case "\n": case " ":
        process.stdout.write(DOWN(menuH) + "\n");
        return displayItems[sel].isBack ? null : displayItems[sel];
      case "escape": case "q":
        process.stdout.write(DOWN(menuH) + "\n");
        return null;
      default:
        if (key >= "1" && key <= "9") {
          const idx = parseInt(key) - 1;
          if (idx < displayItems.length) {
            process.stdout.write(DOWN(menuH) + "\n");
            return displayItems[idx].isBack ? null : displayItems[idx];
          }
        }
        continue;
    }
    render();
  }
}

// ─── Text input with raw keyboard ─────────────────────────

async function textInput(prompt, def = "") {
  process.stdout.write(color("  " + prompt + " ", C.bold));
  if (def) process.stdout.write(color(`[${def}]`, C.dim));

  let text = "", pos = 0;

  function redraw() {
    process.stdout.write("\r" + CLEAR_LINE);
    process.stdout.write(color("  " + prompt + " ", C.bold));
    const display = text || (def ? color(def, C.dim) : "");
    process.stdout.write(display);
    const col = prompt.length + 3 + pos;
    process.stdout.write(`\x1b[${col}G`);
  }

  redraw();

  while (true) {
    const { key, raw } = await readKey();
    switch (key) {
      case "\r": case "\n": process.stdout.write("\n"); return text || def || "";
      case "escape": process.stdout.write("\n"); return "";
      case "\x7f":
        if (pos > 0) { text = text.slice(0, pos - 1) + text.slice(pos); pos--; } break;
      case "left":  if (pos > 0) pos--; break;
      case "right": if (pos < text.length) pos++; break;
      case "home": pos = 0; break;
      case "end": pos = text.length; break;
      case "\x15": text = ""; pos = 0; break; // Ctrl+U
      case "\x17": { // Ctrl+W
        const before = text.slice(0, pos);
        const m = before.match(/(.*\s)?\S+$/);
        const start = m ? before.length - m[0].length : 0;
        text = text.slice(0, start) + text.slice(pos);
        pos = start;
        break;
      }
      default:
        if (raw && raw.length === 1 && raw.charCodeAt(0) >= 32 && raw.charCodeAt(0) < 127) {
          text = text.slice(0, pos) + raw + text.slice(pos);
          pos++;
        }
    }
    redraw();
  }
}

// ─── Confirm dialog ───────────────────────────────────────

async function confirmDialog(msg) {
  process.stdout.write("\n" + color("  " + msg, C.yellow + C.bold) + "\n");
  const choice = await interactiveMenu([
    { label: "Yes", value: true },
    { label: "No", value: false },
  ], { showBack: false });
  return choice?.value ?? false;
}

async function pressAnyKey(msg = "Press any key to continue...") {
  process.stdout.write("\n  " + color(msg, C.dim));
  await keypress();
  process.stdout.write("\n");
}

// ─── Header ───────────────────────────────────────────────

function renderHeader(config) {
  const n = Object.keys(config.profiles || {}).length;
  process.stdout.write("\n  Profiles: " + color(String(n), C.cyan) + " | Current: " + color(config.current || "none", C.green) + "\n");
  const p = config.settings.proxy || {};
  if (p.target) {
    process.stdout.write("  Proxy: " + color(p.target, C.magenta) + (p.failover?.length ? " → " + p.failover.join(" → ") : "") + "\n");
  }
}

function fmtProfiles(config) {
  const names = Object.keys(config.profiles || {});
  if (names.length === 0) return [color("  No profiles configured.", C.dim)];
  return names.map((n) => {
    const p = config.profiles[n];
    const mark = config.current === n ? "* " : "  ";
    const tag = p.proxy ? color(" [proxy]", C.magenta) : "";
    const models = (p.models || []).map((m) => m.id).join(", ");
    return mark + color(n, C.bold) + tag + "\n    " + color(p.api, C.dim) + " | " + color(p.baseUrl, C.dim) + "\n    models: " + color(models, C.cyan);
  });
}

// ─── Screens ──────────────────────────────────────────────

async function screenMain(config) {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n");
  renderHeader(config);
  process.stdout.write("\n" + divider("Main Menu") + "\n");

  return interactiveMenu([
    { label: "Profiles", desc: "View and manage provider profiles", action: "profiles" },
    { label: "Proxy", desc: "Manage local proxy, target, failover, daemon", action: "proxy" },
    { label: "Stats", desc: "View request statistics and usage", action: "stats" },
    { label: "Sync", desc: "Export/import encrypted config", action: "sync" },
    { label: "Open dashboard", desc: "Open provider's API key management page", action: "open" },
    { label: "Doctor", desc: "Run diagnostics", action: "doctor" },
    { label: "Exit", desc: "Quit pi-switch TUI", action: "exit" },
  ], { showBack: false });
}

async function screenProfiles(config) {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n");
  process.stdout.write("\n" + divider("Profiles") + "\n");
  process.stdout.write(fmtProfiles(config).join("\n") + "\n\n");
  process.stdout.write("  Current: " + color(config.current || "none", C.green) + "\n");
  process.stdout.write("  Profiles: " + color(String(Object.keys(config.profiles || {}).length), C.cyan) + "\n");

  return interactiveMenu([
    { label: "Add provider", desc: "Add a new profile from preset or custom endpoint", action: "add" },
    { label: "Edit provider", desc: "Edit API key, base URL, or models", action: "edit" },
    { label: "Switch active profile", desc: "Activate a different provider", action: "use" },
    { label: "Delete provider", desc: "Remove a profile", action: "delete" },
    { label: "Show provider details", desc: "View full profile config", action: "show" },
    { label: "Presets list", desc: "View available presets", action: "presets" },
  ], { title: "Profile Actions" });
}

async function screenAddProfile() {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n\n" + divider("Add Provider") + "\n");

  const presets = listPresets();
  const items = presets.map((p) => ({ label: p.id + color(" — " + p.description, C.dim), value: p.id }));
  items.push(
    { label: color("Custom OpenAI-compatible", C.dim), desc: "Custom endpoint with OpenAI API", value: "custom-openai" },
    { label: color("Custom Anthropic-compatible", C.dim), desc: "Custom endpoint with Anthropic API", value: "custom-anthropic" },
  );

  const choice = await interactiveMenu(items, { title: "Choose preset" });
  if (!choice) return null;

  const presetId = choice.value;
  const isCustom = presetId === "custom-openai" || presetId === "custom-anthropic";
  const defName = isCustom ? (presetId === "custom-openai" ? "custom-openai" : "custom-anthropic") : presetId;
  const name = await textInput("Provider name", defName);
  if (!name) return null;

  let api, baseUrl, model;
  if (isCustom) {
    api = presetId === "custom-openai" ? "openai" : "anthropic";
    baseUrl = await textInput("Base URL"); if (!baseUrl) return null;
    model = await textInput("Model ID"); if (!model) return null;
  }

  const defKey = isCustom ? "$MY_API_KEY" : `$${presetId.toUpperCase().replace(/-/g, "_")}_API_KEY`;
  const apiKey = await textInput("API key ($ENV or literal)", defKey);
  if (!apiKey && !defKey) return null;

  const argv = isCustom
    ? [name, "--api", api, "--base-url", baseUrl, "--api-key", apiKey || defKey, "--model", model]
    : [name, "--preset", presetId, "--api-key", apiKey || defKey];

  const result = await add(argv);
  process.stdout.write(color("\n  ✓ Created '" + result.name + "'", C.green) + "\n");

  if (await confirmDialog("Activate '" + result.name + "' now?")) {
    await use([result.name]);
    process.stdout.write(color("  ✓ Activated '" + result.name + "'", C.green) + "\n");
  }
  await pressAnyKey();
  return true;
}

async function screenEditProfile(config) {
  const names = Object.keys(config.profiles || {});
  if (names.length === 0) {
    process.stdout.write("\n" + color("  No profiles to edit.", C.yellow) + "\n");
    await pressAnyKey(); return;
  }
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n");

  const choice = await interactiveMenu(names.map((n) => ({ label: n, value: n })), { title: "Select profile to edit" });
  if (!choice) return;
  const name = choice.value, profile = config.profiles[name];

  const action = await interactiveMenu([
    { label: "Edit API key", desc: profile.apiKey, action: "key" },
    { label: "Edit Base URL", desc: profile.baseUrl, action: "url" },
    { label: "Edit models", desc: (profile.models || []).map((m) => m.id).join(", "), action: "models" },
  ], { title: "Edit " + name });
  if (!action) return;

  if (action.action === "key") {
    const v = await textInput("New API key", profile.apiKey);
    if (!v) return;
    await update(name, { apiKey: v });
    process.stdout.write(color("\n  ✓ API key updated", C.green) + "\n");
  } else if (action.action === "url") {
    const v = await textInput("New Base URL", profile.baseUrl);
    if (!v) return;
    await update(name, { baseUrl: v });
    process.stdout.write(color("\n  ✓ Base URL updated", C.green) + "\n");
  } else if (action.action === "models") {
    const cur = (profile.models || []).map((m) => m.name ? `${m.id}=${m.name}` : m.id).join(", ");
    const v = await textInput("Models (comma separated)", cur);
    if (!v) return;
    const models = v.split(",").map((s) => s.trim()).filter(Boolean).map((item) => {
      const idx = item.indexOf("=");
      const id = idx === -1 ? item : item.slice(0, idx).trim();
      const mn = idx === -1 ? undefined : item.slice(idx + 1).trim();
      return { id, ...(mn ? { name: mn } : {}), input: ["text"], contextWindow: 1000000, maxTokens: 128000, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } };
    });
    await update(name, { models });
    process.stdout.write(color("\n  ✓ Models updated", C.green) + "\n");
  }
  await pressAnyKey();
}

async function screenUseProfile(config) {
  const names = Object.keys(config.profiles || {});
  if (names.length === 0) {
    process.stdout.write("\n" + color("  No profiles.", C.yellow) + "\n");
    await pressAnyKey(); return;
  }
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n");

  const choice = await interactiveMenu(
    names.map((n) => ({ label: n + (config.current === n ? color("  (current)", C.green) : ""), value: n })),
    { title: "Select profile to activate" },
  );
  if (!choice) return;
  const result = await use([choice.value]);
  process.stdout.write(color("\n  ✓ Activated '" + result.name + "'", C.green) + "\n");
  await pressAnyKey();
}

async function screenDeleteProfile(config) {
  const names = Object.keys(config.profiles || {});
  if (names.length === 0) {
    process.stdout.write("\n" + color("  No profiles.", C.yellow) + "\n");
    await pressAnyKey(); return;
  }
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n");

  const choice = await interactiveMenu(names.map((n) => ({ label: n, value: n })), { title: "Select profile to delete" });
  if (!choice) return;
  if (!(await confirmDialog("Really delete '" + choice.value + "'?"))) {
    process.stdout.write(color("  Cancelled.", C.yellow) + "\n");
    await pressAnyKey(); return;
  }
  await remove(choice.value);
  process.stdout.write(color("\n  ✓ Deleted '" + choice.value + "'", C.green) + "\n");
  await pressAnyKey();
}

async function screenShowProfile(config) {
  const names = Object.keys(config.profiles || {});
  if (names.length === 0) {
    process.stdout.write("\n" + color("  No profiles.", C.yellow) + "\n");
    await pressAnyKey(); return;
  }
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n");

  const choice = await interactiveMenu(names.map((n) => ({ label: n, value: n })), { title: "Select profile" });
  if (!choice) return;
  const p = config.profiles[choice.value];
  process.stdout.write("\n  Name: " + color(choice.value, C.bold) + "\n");
  process.stdout.write("  API: " + p.api + "\n");
  process.stdout.write("  Base URL: " + p.baseUrl + "\n");
  process.stdout.write("  API Key: " + p.apiKey + "\n");
  process.stdout.write("  Preset: " + (p.preset || "custom") + "\n");
  process.stdout.write("  Models:\n");
  (p.models || []).forEach((m) => {
    process.stdout.write("    - " + color(m.id, C.cyan) + (m.name ? " (" + m.name + ")" : "") + "\n");
  });
  await pressAnyKey();
}

async function screenPresetsList() {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n\n" + divider("Presets") + "\n\n");
  for (const p of listPresets()) {
    process.stdout.write("  " + color(p.id, C.bold) + "\n");
    process.stdout.write("    " + color(p.description, C.dim) + "\n");
    process.stdout.write("    " + p.api + " → " + p.baseUrl + "\n");
    process.stdout.write("    models: " + color(p.models.map((m) => m.id).join(", "), C.cyan) + "\n\n");
  }
  await pressAnyKey();
}

async function screenProxy(config) {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n\n" + divider("Proxy Management") + "\n");

  const proxy = config.settings.proxy || {};
  const tgt = proxy.target;
  const tgtProfile = tgt ? config.profiles[tgt] : null;

  process.stdout.write("  Target: " + color(tgt || "none", tgt ? C.green : C.yellow) + "\n");
  if (tgtProfile) process.stdout.write("    " + color(tgtProfile.api, C.dim) + " | " + color(tgtProfile.baseUrl, C.dim) + "\n");
  process.stdout.write("  Failover: " + color((proxy.failover || []).join(" → ") || "none", C.cyan) + "\n");
  process.stdout.write("  Addr: " + color((proxy.host || "127.0.0.1") + ":" + (proxy.port || 43112), C.cyan) + "\n");
  process.stdout.write("  Circuit: " + color(proxy.circuitBreaker?.enabled ? "enabled" : "disabled", proxy.circuitBreaker?.enabled ? C.green : C.yellow) + "\n");

  try {
    const ds = await daemonStatus();
    process.stdout.write("\n  Daemon: " + (ds.running ? color("● RUNNING (pid " + ds.pid + ")", C.green) : color("○ stopped", C.yellow)) + "\n");
    if (ds.running) process.stdout.write("    " + ds.host + ":" + ds.port + (ds.startedAt ? " | since " + new Date(ds.startedAt).toLocaleString() : "") + "\n");
  } catch { process.stdout.write("\n  Daemon: " + color("unknown", C.yellow) + "\n"); }

  return interactiveMenu([
    { label: "Install proxy provider", desc: "Create a proxy profile in pi config", action: "install" },
    { label: "Set target profile", desc: "Choose which profile the proxy forwards to", action: "target" },
    { label: "Set failover", desc: "Configure failover chain", action: "failover" },
    { label: "Daemon: Start", desc: "Start proxy in background", action: "daemon-start" },
    { label: "Daemon: Stop", desc: "Stop background proxy", action: "daemon-stop" },
    { label: "Circuit: Status", desc: "View circuit breaker state", action: "circuit-status" },
    { label: "Circuit: Reset", desc: "Reset circuit breaker", action: "circuit-reset" },
  ], { title: "Proxy Actions" });
}

async function screenStats() {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n\n" + divider("Usage Statistics") + "\n");
  const fmt = await interactiveMenu([
    { label: "Summary", desc: "Overview: total, success, fail, latency", value: "summary" },
    { label: "By Provider", desc: "Per provider breakdown", value: "by-provider" },
    { label: "By Model", desc: "Per model breakdown", value: "by-model" },
    { label: "Hourly", desc: "Hourly distribution chart", value: "hourly" },
    { label: "Full Report", desc: "All details", value: "full" },
  ], { title: "Report Format" });
  if (!fmt) return;
  process.stdout.write("\n" + formatStatsText(await getStats(), fmt.value) + "\n");
  await pressAnyKey();
}

async function screenSync() {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n\n" + divider("Config Sync") + "\n");
  const action = await interactiveMenu([
    { label: "Export config", desc: "Encrypt and export config to file", action: "export" },
    { label: "Import config", desc: "Decrypt and import config from file", action: "import" },
    { label: "View export info", desc: "See where export files are stored", action: "info" },
  ], { title: "Sync Actions" });
  if (!action) return;

  try {
    if (action.action === "export") {
      const pw = await textInput("Passphrase (min 8 chars)");
      if (!pw) return;
      process.stdout.write(color("\n  ✓ " + (await exportConfig(pw)).message, C.green) + "\n");
    } else if (action.action === "import") {
      const fp = await textInput("Export file path"); if (!fp) return;
      const pw = await textInput("Passphrase"); if (!pw) return;
      const r = await importConfig(fp, pw);
      process.stdout.write(color("\n  ✓ " + r.message, C.green) + "\n");
      if (r.sanitizedKeys) process.stdout.write(color("  Sanitized: " + r.sanitizedKeys + " keys → env vars", C.yellow) + "\n");
    } else if (action.action === "info") {
      process.stdout.write("\n  Export dir: " + color(process.env.HOME + "/.pi-switch/exports", C.cyan) + "\n");
      process.stdout.write("  Format: AES-256-CBC encrypted JSON\n");
      process.stdout.write("  API keys → $ENV placeholders on import\n");
    }
  } catch (err) { process.stdout.write(color("\n  Error: " + err.message, C.red) + "\n"); }
  await pressAnyKey();
}

async function screenOpen(config) {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n\n" + divider("Open Dashboard") + "\n");
  const items = [];
  for (const p of ["openrouter", "anthropic", "deepseek", "siliconflow", "openai"])
    items.push({ label: color(p, C.cyan) + "  (preset)", desc: "API key management page", value: p });
  for (const [n, p] of Object.entries(config.profiles || {}))
    if (!p.proxy) items.push({ label: n + "  (profile)", desc: p.baseUrl, value: n });

  const choice = await interactiveMenu(items, { title: "Choose provider to open" });
  if (!choice) return;
  const r = await openProvider(choice.value);
  process.stdout.write("\n");
  if (r?.opened) process.stdout.write(color("  ✓ Opened " + r.label, C.green) + "\n");
  else if (r?.url) process.stdout.write(color("  " + r.label + ": " + r.url, C.cyan) + "\n");
  else process.stdout.write(color("  No link for '" + choice.value + "'", C.yellow) + "\n");
  await pressAnyKey();
}

async function screenDoctor() {
  process.stdout.write(CLEAR_SCREEN);
  process.stdout.write(banner() + "\n\n" + divider("Doctor") + "\n\n");
  const { doctor } = await import("./commands.js");
  for (const c of await doctor())
    process.stdout.write("  " + (c.ok ? color("✓", C.green) : color("✗", C.red)) + " " + c.msg + "\n");
  await pressAnyKey();
}

// ─── Proxy sub-actions ────────────────────────────────────

async function doProxyInstall() {
  const name = await textInput("Provider name", "proxy");
  if (!name) return;
  process.stdout.write(color("\n  ✓ Installed '" + (await installProxyProvider(name ? [name] : [])).name + "'", C.green) + "\n");
}

async function doProxyTarget(config) {
  const names = Object.keys(config.profiles || {}).filter((n) => !config.profiles[n]?.proxy);
  if (names.length === 0) { process.stdout.write(color("\n  No non-proxy profiles.", C.yellow) + "\n"); await pressAnyKey(); return null; }
  process.stdout.write(CLEAR_SCREEN); process.stdout.write(banner() + "\n");
  const choice = await interactiveMenu(names.map((n) => ({ label: n, value: n })), { title: "Select proxy target" });
  if (!choice) return null;
  const others = names.filter((n) => n !== choice.value);
  const ft = others.length ? await textInput("Failover (comma sep, optional)", others.join(",")) : "";
  const failover = ft ? ft.split(",").map((s) => s.trim()).filter(Boolean) : [];
  const r = await setProxyTarget(choice.value, failover.length ? failover : undefined);
  process.stdout.write(color("\n  ✓ Target: " + r.target + (r.failover.length ? " → " + r.failover.join(" → ") : ""), C.green) + "\n");
  return r;
}

async function doProxyFailover(config) {
  const px = config.settings.proxy || {};
  if (!px.target) { process.stdout.write(color("\n  Set a target first.", C.yellow) + "\n"); await pressAnyKey(); return; }
  const names = Object.keys(config.profiles || {}).filter((n) => n !== px.target && !config.profiles[n]?.proxy);
  const def = px.failover?.join(",") || "";
  const ft = await textInput("Failover profiles", def);
  const failover = (ft || def).split(",").map((s) => s.trim()).filter(Boolean);
  const r = await setProxyTarget(px.target, failover.length ? failover : []);
  process.stdout.write(color("\n  ✓ Failover: " + (r.failover.join(" → ") || "none"), C.green) + "\n");
}

// ─── Entry ────────────────────────────────────────────────

export async function runTui() {
  await init();
  process.on("SIGINT", () => { rawOff(); process.stdout.write(color("\n\n  Goodbye!", C.cyan) + "\n"); process.exit(0); });

  let config = await loadConfig();

  while (true) {
    const main = await screenMain(config);
    if (!main || main.action === "exit") break;

    rawOn();
    try {
      switch (main.action) {
        case "profiles": {
          const a = await screenProfiles(config);
          if (a) {
            switch (a.action) {
              case "add": await screenAddProfile(); break;
              case "edit": await screenEditProfile(config); break;
              case "use": await screenUseProfile(config); break;
              case "delete": await screenDeleteProfile(config); break;
              case "show": await screenShowProfile(config); break;
              case "presets": await screenPresetsList(); break;
            }
          }
          config = await loadConfig(); break;
        }
        case "proxy": {
          const a = await screenProxy(config);
          if (a) {
            switch (a.action) {
              case "install": await doProxyInstall(); break;
              case "target": await doProxyTarget(config); break;
              case "failover": await doProxyFailover(config); break;
              case "daemon-start": process.stdout.write(color("\n  ✓ " + (await daemonStart({})).message, C.green) + "\n"); break;
              case "daemon-stop": process.stdout.write(color("\n  " + (await daemonStop()).message, C.yellow) + "\n"); break;
              case "circuit-status": process.stdout.write("\n" + JSON.stringify(await getCircuitState(), null, 2) + "\n"); break;
              case "circuit-reset": await resetCircuitState(); process.stdout.write(color("\n  ✓ Circuit reset", C.green) + "\n"); break;
            }
            if (a.action !== "circuit-status") await pressAnyKey();
          }
          config = await loadConfig(); break;
        }
        case "stats": await screenStats(); break;
        case "sync": await screenSync(); break;
        case "open": await screenOpen(config); break;
        case "doctor": await screenDoctor(); break;
      }
    } catch (err) {
      process.stdout.write("\n" + color("  Error: " + err.message, C.red) + "\n");
      await pressAnyKey();
    }
  }

  rawOff();
  process.stdout.write(color("\n  Goodbye!", C.cyan) + "\n");
}
