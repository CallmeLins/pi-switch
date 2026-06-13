import http from "node:http";
import { spawn } from "node:child_process";
import { mkdir, appendFile, readFile, writeFile, unlink } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import { CONFIG_DIR, loadConfig } from "./core.js";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 43112;
const RETRY_STATUSES = new Set([429, 500, 502, 503, 504]);
const CIRCUIT_PATH = join(CONFIG_DIR, "circuit.json");
const PID_PATH = join(CONFIG_DIR, "proxy.pid");
const DAEMON_LOG_PATH = join(CONFIG_DIR, "proxy-daemon.log");

function resolveEnvLike(value) {
  if (typeof value !== "string") return value;
  const exact = value.match(/^\$\{?([A-Z0-9_]+)\}?$/);
  if (exact) return process.env[exact[1]] || value;
  return value.replace(/\$\{?([A-Z0-9_]+)\}?/g, (_, key) => process.env[key] || "");
}

function joinUrl(baseUrl, suffix) {
  return `${baseUrl.replace(/\/+$/, "")}/${suffix.replace(/^\/+/, "")}`;
}

async function readBody(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(Buffer.from(chunk));
  return Buffer.concat(chunks);
}

async function logRequest(entry) {
  await mkdir(CONFIG_DIR, { recursive: true });
  await appendFile(join(CONFIG_DIR, "requests.log"), `${JSON.stringify({ ts: new Date().toISOString(), ...entry })}\n`);
}

function sendJson(res, status, payload) {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(payload));
}

function stripHopByHopHeaders(headers) {
  const out = { ...headers };
  for (const key of ["host", "connection", "content-length", "transfer-encoding", "upgrade", "keep-alive", "proxy-authenticate", "proxy-authorization", "te", "trailer"]) {
    delete out[key];
  }
  return out;
}

function pickDefaultTarget(config) {
  return Object.keys(config.profiles || {}).find((name) => !config.profiles[name]?.proxy) || null;
}

function buildCandidateNames(config, explicitProfile) {
  const names = [];
  const add = (name) => {
    if (!name || names.includes(name)) return;
    const profile = config.profiles?.[name];
    if (!profile || profile.proxy) return;
    names.push(name);
  };
  add(explicitProfile);
  add(config.settings.proxy.target);
  for (const name of config.settings.proxy.failover || []) add(name);
  add(pickDefaultTarget(config));
  return names;
}

function shouldRetryStatus(status) {
  return RETRY_STATUSES.has(status);
}

async function fetchOpenAI(profile, req, body, targetPath) {
  const upstreamUrl = joinUrl(profile.baseUrl, targetPath);
  const headers = stripHopByHopHeaders(req.headers);
  headers.authorization = `Bearer ${resolveEnvLike(profile.apiKey)}`;
  headers["content-type"] = headers["content-type"] || "application/json";
  if (profile.headers) {
    for (const [key, value] of Object.entries(profile.headers)) headers[key.toLowerCase()] = resolveEnvLike(value);
  }

  const response = await fetch(upstreamUrl, {
    method: req.method,
    headers,
    body: req.method === "GET" || req.method === "HEAD" ? undefined : body,
  });
  return { response, upstreamUrl };
}

// ---------- OpenAI <-> Anthropic conversion helpers ----------

function openAIMessagesToAnthropic(messages) {
  // Extract system messages and conversation messages
  const systemMessages = [];
  const conversationMessages = [];
  for (const msg of messages) {
    if (msg.role === "system") {
      systemMessages.push(msg);
    } else {
      conversationMessages.push(msg);
    }
  }

  const system = systemMessages.length > 0
    ? systemMessages.map((m) => {
        if (typeof m.content === "string") return { type: "text", text: m.content };
        return { type: "text", text: JSON.stringify(m.content) };
      })
    : undefined;

  const anthropicMessages = conversationMessages.map((msg) => {
    const role = msg.role === "assistant" ? "assistant" : "user";
    if (typeof msg.content === "string") {
      return { role, content: [{ type: "text", text: msg.content }] };
    }
    if (Array.isArray(msg.content)) {
      const parts = [];
      for (const part of msg.content) {
        if (part.type === "text") {
          parts.push({ type: "text", text: part.text });
        } else if (part.type === "image_url" && part.image_url?.url) {
          const url = part.image_url.url;
          const match = url.match(/^data:([^;]+);base64,(.+)$/);
          if (match) {
            parts.push({
              type: "image",
              source: { type: "base64", media_type: match[1], data: match[2] }
            });
          }
        } else {
          parts.push({ type: "text", text: JSON.stringify(part) });
        }
      }
      return { role, content: parts };
    }
    return { role, content: [{ type: "text", text: String(msg.content) }] };
  });

  return { system, messages: anthropicMessages };
}

function openAIToAnthropicBody(openaiBody) {
  const { model, messages, max_tokens, temperature, top_p, stop, stream } = openaiBody || {};
  const { system, messages: anthropicMessages } = openAIMessagesToAnthropic(messages || []);

  const anthropicBody = {
    model: model || "claude-sonnet-4-5",
    max_tokens: max_tokens || 16384,
    messages: anthropicMessages,
  };

  if (system) anthropicBody.system = system;
  if (temperature !== undefined) anthropicBody.temperature = temperature;
  if (top_p !== undefined) anthropicBody.top_p = top_p;
  if (stop) {
    anthropicBody.stop_sequences = Array.isArray(stop) ? stop : [stop];
  }
  if (stream) anthropicBody.stream = true;

  return anthropicBody;
}

function anthropicToOpenAIResponse(anthropicResponse) {
  return {
    id: anthropicResponse.id || `chatcmpl-${Date.now()}`,
    object: "chat.completion",
    created: Math.floor(Date.now() / 1000),
    model: anthropicResponse.model || "claude-sonnet-4-5",
    choices: (anthropicResponse.content || []).map((block, index) => ({
      index,
      message: {
        role: "assistant",
        content: block.type === "text" ? block.text : JSON.stringify(block),
      },
      finish_reason: anthropicResponse.stop_reason === "end_turn" ? "stop"
        : anthropicResponse.stop_reason === "max_tokens" ? "length"
        : anthropicResponse.stop_reason || "stop",
    })),
    usage: anthropicResponse.usage ? {
      prompt_tokens: anthropicResponse.usage.input_tokens || 0,
      completion_tokens: anthropicResponse.usage.output_tokens || 0,
      total_tokens: (anthropicResponse.usage.input_tokens || 0) + (anthropicResponse.usage.output_tokens || 0),
    } : undefined,
  };
}

async function fetchAnthropic(profile, req, body, targetPath) {
  const upstreamUrl = joinUrl(profile.baseUrl, targetPath);
  const headers = stripHopByHopHeaders(req.headers);
  headers["x-api-key"] = resolveEnvLike(profile.apiKey);
  headers["anthropic-version"] = headers["anthropic-version"] || "2023-06-01";
  headers["content-type"] = "application/json";
  if (profile.headers) {
    for (const [key, value] of Object.entries(profile.headers)) headers[key.toLowerCase()] = resolveEnvLike(value);
  }

  const response = await fetch(upstreamUrl, {
    method: req.method,
    headers,
    body: req.method === "GET" || req.method === "HEAD" ? undefined : body,
  });
  return { response, upstreamUrl };
}

async function writeUpstreamResponse(res, upstream) {
  res.writeHead(upstream.status, Object.fromEntries(upstream.headers.entries()));
  if (upstream.body) {
    const reader = upstream.body.getReader();
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      res.write(Buffer.from(value));
    }
    res.end();
  } else {
    res.end();
  }
}

async function readCircuitState() {
  try {
    return JSON.parse(await readFile(CIRCUIT_PATH, "utf8"));
  } catch {
    return { providers: {} };
  }
}

async function writeCircuitState(state) {
  await mkdir(CONFIG_DIR, { recursive: true });
  await writeFile(CIRCUIT_PATH, `${JSON.stringify(state, null, 2)}\n`, "utf8");
}

function isCircuitOpen(state, name, settings) {
  if (!settings?.enabled) return false;
  const entry = state.providers?.[name];
  if (!entry?.openedAt) return false;
  const cooldownMs = Number(settings.cooldownSeconds || 60) * 1000;
  return Date.now() - entry.openedAt < cooldownMs;
}

async function recordCircuitSuccess(name) {
  const state = await readCircuitState();
  state.providers ??= {};
  state.providers[name] = { failures: 0, openedAt: null, lastSuccessAt: Date.now() };
  await writeCircuitState(state);
}

async function recordCircuitFailure(name, settings, reason) {
  if (!settings?.enabled) return;
  const state = await readCircuitState();
  state.providers ??= {};
  const entry = state.providers[name] || { failures: 0, openedAt: null };
  entry.failures = (entry.failures || 0) + 1;
  entry.lastFailureAt = Date.now();
  entry.lastError = reason;
  if (entry.failures >= Number(settings.failureThreshold || 3)) {
    entry.openedAt = Date.now();
  }
  state.providers[name] = entry;
  await writeCircuitState(state);
}

export async function getCircuitState() {
  return readCircuitState();
}

export async function resetCircuitState() {
  const state = { providers: {} };
  await writeCircuitState(state);
  return state;
}

// ---------- Daemon management ----------

async function readPidFile() {
  try {
    const text = await readFile(PID_PATH, "utf8");
    return JSON.parse(text);
  } catch {
    return null;
  }
}

async function writePidFile(info) {
  await mkdir(CONFIG_DIR, { recursive: true });
  await writeFile(PID_PATH, `${JSON.stringify(info)}\n`, "utf8");
}

async function removePidFile() {
  try { await unlink(PID_PATH); } catch {}
}

function isProcessAlive(pid) {
  if (!pid || !Number.isFinite(pid)) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function resolvePiSwitchBin() {
  // Try to find pi-switch relative to this module or via known paths
  const candidates = [
    join(homedir(), "WorkSpace/Learn/pi-switch/bin/pi-switch.js"),
    join(process.cwd(), "bin/pi-switch.js"),
    process.argv[1],
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  // Fall back to the current script
  return process.argv[1] || "pi-switch";
}

export async function daemonStart(options = {}) {
  const info = await readPidFile();
  if (info?.pid && isProcessAlive(info.pid)) {
    return { running: true, pid: info.pid, host: info.host, port: info.port, message: `Proxy daemon already running (PID ${info.pid}) on http://${info.host}:${info.port}` };
  }

  // Clean up stale PID file
  await removePidFile();

  const config = await loadConfig();
  const host = options.host || config.settings.proxy.host || "127.0.0.1";
  const port = options.port || config.settings.proxy.port || "43112";

  const binPath = resolvePiSwitchBin();
  const logFd = await (async () => {
    const { open } = await import("node:fs/promises");
    await mkdir(CONFIG_DIR, { recursive: true });
    return open(DAEMON_LOG_PATH, "a");
  })();

  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [
      binPath,
      "proxy", "start",
      "--host", host,
      "--port", String(port),
    ], {
      detached: true,
      stdio: ["ignore", logFd.fd, logFd.fd],
      env: { ...process.env },
    });

    child.on("error", (err) => {
      reject(err);
    });

    child.on("exit", (code) => {
      if (code !== null && code !== 0) {
        reject(new Error(`Daemon process exited with code ${code}`));
      }
    });

    // Wait a moment for the proxy to start, then check
    setTimeout(async () => {
      child.unref();
      const childPid = child.pid;
      if (childPid && isProcessAlive(childPid)) {
        await writePidFile({ pid: childPid, host, port: Number(port), startedAt: Date.now() });
        resolve({
          running: true,
          pid: childPid,
          host,
          port: Number(port),
          logPath: DAEMON_LOG_PATH,
          message: `Proxy daemon started (PID ${childPid}) on http://${host}:${port}`,
        });
      } else {
        reject(new Error("Daemon process failed to start"));
      }
    }, 1000);
  });
}

export async function daemonStop() {
  const info = await readPidFile();
  if (!info?.pid) {
    return { running: false, message: "No proxy daemon PID file found" };
  }

  if (!isProcessAlive(info.pid)) {
    await removePidFile();
    return { running: false, message: `PID ${info.pid} is not alive (cleaned up stale PID file)` };
  }

  // Try graceful shutdown first
  try {
    process.kill(info.pid, "SIGTERM");
  } catch {
    await removePidFile();
    return { running: false, message: `Failed to signal PID ${info.pid}` };
  }

  // Wait for process to exit, up to 5 seconds
  for (let i = 0; i < 50; i++) {
    await new Promise((r) => setTimeout(r, 100));
    if (!isProcessAlive(info.pid)) {
      await removePidFile();
      return { running: false, pid: info.pid, message: `Proxy daemon (PID ${info.pid}) stopped` };
    }
  }

  // Force kill
  try { process.kill(info.pid, "SIGKILL"); } catch {}
  await removePidFile();
  return { running: false, pid: info.pid, message: `Proxy daemon (PID ${info.pid}) force killed` };
}

export async function daemonStatus() {
  const info = await readPidFile();
  if (!info?.pid) {
    return { running: false, pid: null, message: "Proxy daemon is not running (no PID file)" };
  }

  const alive = isProcessAlive(info.pid);
  if (!alive) {
    await removePidFile();
    return { running: false, pid: info.pid, message: `Proxy daemon is not running (PID ${info.pid} is dead)` };
  }

  // Use actual host/port from PID file, fall back to config
  const host = info.host || "127.0.0.1";
  const port = info.port || 43112;

  const config = await loadConfig();

  return {
    running: true,
    pid: info.pid,
    host,
    port,
    target: config.settings.proxy.target,
    failover: config.settings.proxy.failover || [],
    logPath: DAEMON_LOG_PATH,
    startedAt: info.startedAt,
    message: `Proxy daemon running (PID ${info.pid}) on http://${host}:${port}`,
  };
}

async function forwardWithFailover(req, res, config, candidateNames, targetPath) {
  const started = Date.now();
  const body = await readBody(req);
  let parsed;
  try {
    parsed = JSON.parse(body.toString("utf8") || "{}");
  } catch {
    parsed = null;
  }

  const attempts = [];
  const circuitState = await readCircuitState();
  const circuitSettings = config.settings.proxy.circuitBreaker;
  let lastRetryResponse = null;
  let lastRetryMeta = null;

  for (const name of candidateNames) {
    const profile = config.profiles[name];
    if (!profile) continue;
    if (isCircuitOpen(circuitState, name, circuitSettings)) {
      attempts.push({ provider: name, skipped: true, reason: "circuit_open" });
      await logRequest({ ok: false, skipped: true, provider: name, model: parsed?.model, error: "circuit_open", ms: Date.now() - started });
      continue;
    }

    const isAnthropic = profile.api === "anthropic-messages";
    const isOpenAI = profile.api === "openai-completions";
    if (!isAnthropic && !isOpenAI) {
      attempts.push({ provider: name, skipped: true, reason: `unsupported api ${profile.api}` });
      await logRequest({ ok: false, skipped: true, provider: name, model: parsed?.model, error: `unsupported api ${profile.api}`, ms: Date.now() - started });
      continue;
    }

    try {
      let response, upstreamUrl;

      if (isAnthropic) {
        // Convert OpenAI-format body to Anthropic format
        const anthropicBody = openAIToAnthropicBody(parsed || {});
        const anthropicReq = {
          method: "POST",
          headers: { ...req.headers, "content-type": "application/json" },
        };
        const { response: upstreamResp, upstreamUrl: upstreamUrlResp } = await fetchAnthropic(
          profile, anthropicReq, Buffer.from(JSON.stringify(anthropicBody)), "messages"
        );
        response = upstreamResp;
        upstreamUrl = upstreamUrlResp;

        // Convert response back to OpenAI format if successful
        if (response.ok) {
          const anthropicData = await response.json();
          const openaiData = anthropicToOpenAIResponse(anthropicData);
          attempts.push({ provider: name, status: 200, upstreamUrl, converted: "anthropic->openai" });
          sendJson(res, 200, openaiData);
          await recordCircuitSuccess(name);
          await logRequest({ ok: true, provider: name, status: 200, upstreamUrl, model: parsed?.model, converted: "anthropic->openai", attempts, ms: Date.now() - started });
          return;
        }

        // Non-ok response, check if retryable
        const status = response.status;
        attempts.push({ provider: name, status, upstreamUrl });
        if (shouldRetryStatus(status)) {
          if (lastRetryResponse) {
            try { await lastRetryResponse.arrayBuffer(); } catch {}
          }
          lastRetryResponse = response;
          lastRetryMeta = { provider: name, upstreamUrl };
          await recordCircuitFailure(name, circuitSettings, `HTTP ${status}`);
          await logRequest({ ok: false, retry: true, provider: name, status, upstreamUrl, model: parsed?.model, ms: Date.now() - started });
          continue;
        }

        // Non-retryable error, return as-is
        await writeUpstreamResponse(res, response);
        await logRequest({ ok: false, provider: name, status, upstreamUrl, model: parsed?.model, attempts, ms: Date.now() - started });
        return;
      }

      // OpenAI path
      const { response: openaiResp, upstreamUrl: openaiUrl } = await fetchOpenAI(profile, req, body, targetPath);
      response = openaiResp;
      upstreamUrl = openaiUrl;
      attempts.push({ provider: name, status: response.status, upstreamUrl });

      if (shouldRetryStatus(response.status)) {
        if (lastRetryResponse) {
          try { await lastRetryResponse.arrayBuffer(); } catch {}
        }
        lastRetryResponse = response;
        lastRetryMeta = { provider: name, upstreamUrl };
        await recordCircuitFailure(name, circuitSettings, `HTTP ${response.status}`);
        await logRequest({ ok: false, retry: true, provider: name, status: response.status, upstreamUrl, model: parsed?.model, ms: Date.now() - started });
        continue;
      }

      await writeUpstreamResponse(res, response);
      if (response.ok) await recordCircuitSuccess(name);
      await logRequest({ ok: response.ok, provider: name, status: response.status, upstreamUrl, model: parsed?.model, attempts, ms: Date.now() - started });
      return;
    } catch (err) {
      attempts.push({ provider: name, error: err.message });
      await recordCircuitFailure(name, circuitSettings, err.message);
      await logRequest({ ok: false, retry: true, provider: name, model: parsed?.model, error: err.message, ms: Date.now() - started });
    }
  }

  if (lastRetryResponse) {
    await writeUpstreamResponse(res, lastRetryResponse);
    await logRequest({ ok: false, provider: lastRetryMeta?.provider, status: lastRetryResponse.status, upstreamUrl: lastRetryMeta?.upstreamUrl, model: parsed?.model, attempts, exhausted: true, ms: Date.now() - started });
    return;
  }

  sendJson(res, 502, { error: { message: "All proxy upstream attempts failed", type: "failover_exhausted", attempts } });
  await logRequest({ ok: false, attempts, exhausted: true, model: parsed?.model, ms: Date.now() - started });
}

// Keep old function name as alias for backward compatibility
async function forwardOpenAIWithFailover(req, res, config, candidateNames, targetPath) {
  return forwardWithFailover(req, res, config, candidateNames, targetPath);
}

async function forwardOpenAI(req, res, profile, targetPath) {
  if (profile.api !== "openai-completions") {
    sendJson(res, 501, {
      error: {
        message: `pi-switch proxy MVP currently forwards openai-completions only. Active profile api is '${profile.api}'.`,
        type: "unsupported_api",
      },
    });
    return;
  }

  const started = Date.now();
  const body = await readBody(req);
  let parsed;
  try {
    parsed = JSON.parse(body.toString("utf8") || "{}");
  } catch {
    parsed = null;
  }

  const upstreamUrl = joinUrl(profile.baseUrl, targetPath);
  const headers = stripHopByHopHeaders(req.headers);
  headers.authorization = `Bearer ${resolveEnvLike(profile.apiKey)}`;
  headers["content-type"] = headers["content-type"] || "application/json";
  if (profile.headers) {
    for (const [key, value] of Object.entries(profile.headers)) headers[key.toLowerCase()] = resolveEnvLike(value);
  }

  let upstream;
  try {
    upstream = await fetch(upstreamUrl, {
      method: req.method,
      headers,
      body: req.method === "GET" || req.method === "HEAD" ? undefined : body,
    });
  } catch (err) {
    await logRequest({ ok: false, provider: profile.name, upstreamUrl, model: parsed?.model, error: err.message, ms: Date.now() - started });
    sendJson(res, 502, { error: { message: err.message, type: "upstream_fetch_error" } });
    return;
  }

  res.writeHead(upstream.status, Object.fromEntries(upstream.headers.entries()));
  if (upstream.body) {
    const reader = upstream.body.getReader();
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      res.write(Buffer.from(value));
    }
    res.end();
  } else {
    res.end();
  }

  await logRequest({ ok: upstream.ok, status: upstream.status, upstreamUrl, model: parsed?.model, ms: Date.now() - started });
}

async function forwardAnthropicWithFailover(req, res, config, candidateNames) {
  const started = Date.now();
  const body = await readBody(req);
  let parsed;
  try {
    parsed = JSON.parse(body.toString("utf8") || "{}");
  } catch {
    parsed = null;
  }

  const attempts = [];
  const circuitState = await readCircuitState();
  const circuitSettings = config.settings.proxy.circuitBreaker;
  let lastRetryResponse = null;
  let lastRetryMeta = null;

  for (const name of candidateNames) {
    const profile = config.profiles[name];
    if (!profile) continue;
    if (profile.api !== "anthropic-messages") {
      attempts.push({ provider: name, skipped: true, reason: `unsupported api ${profile.api}` });
      continue;
    }
    if (isCircuitOpen(circuitState, name, circuitSettings)) {
      attempts.push({ provider: name, skipped: true, reason: "circuit_open" });
      await logRequest({ ok: false, skipped: true, provider: name, model: parsed?.model, error: "circuit_open", ms: Date.now() - started });
      continue;
    }

    try {
      const { response, upstreamUrl } = await fetchAnthropic(profile, req, body, "messages");
      attempts.push({ provider: name, status: response.status, upstreamUrl });

      if (shouldRetryStatus(response.status)) {
        if (lastRetryResponse) {
          try { await lastRetryResponse.arrayBuffer(); } catch {}
        }
        lastRetryResponse = response;
        lastRetryMeta = { provider: name, upstreamUrl };
        await recordCircuitFailure(name, circuitSettings, `HTTP ${response.status}`);
        await logRequest({ ok: false, retry: true, provider: name, status: response.status, upstreamUrl, model: parsed?.model, ms: Date.now() - started });
        continue;
      }

      await writeUpstreamResponse(res, response);
      if (response.ok) await recordCircuitSuccess(name);
      await logRequest({ ok: response.ok, provider: name, status: response.status, upstreamUrl, model: parsed?.model, attempts, ms: Date.now() - started });
      return;
    } catch (err) {
      attempts.push({ provider: name, error: err.message });
      await recordCircuitFailure(name, circuitSettings, err.message);
      await logRequest({ ok: false, retry: true, provider: name, model: parsed?.model, error: err.message, ms: Date.now() - started });
    }
  }

  if (lastRetryResponse) {
    await writeUpstreamResponse(res, lastRetryResponse);
    await logRequest({ ok: false, provider: lastRetryMeta?.provider, status: lastRetryResponse.status, upstreamUrl: lastRetryMeta?.upstreamUrl, model: parsed?.model, attempts, exhausted: true, ms: Date.now() - started });
    return;
  }

  sendJson(res, 502, { error: { message: "All Anthropic upstream attempts failed", type: "failover_exhausted", attempts } });
  await logRequest({ ok: false, attempts, exhausted: true, model: parsed?.model, ms: Date.now() - started });
}

export async function startProxy(options = {}) {
  const initialConfig = await loadConfig();
  const host = options.host || initialConfig.settings.proxy.host || DEFAULT_HOST;
  const port = Number(options.port || initialConfig.settings.proxy.port || DEFAULT_PORT);

  const server = http.createServer(async (req, res) => {
    const activeConfig = await loadConfig();
    const activeName = options.profile || activeConfig.settings.proxy.target || pickDefaultTarget(activeConfig);
    const activeProfile = activeName ? activeConfig.profiles[activeName] : null;
    if (!activeProfile) {
      sendJson(res, 503, { error: { message: "No active profile", type: "no_active_profile" } });
      return;
    }
    activeProfile.name = activeName;
    try {
      const url = new URL(req.url || "/", `http://${host}:${port}`);
      if (url.pathname === "/health") {
        const candidates = buildCandidateNames(activeConfig, options.profile);
        const supportedApis = new Set();
        for (const candidate of candidates) {
          const p = activeConfig.profiles[candidate];
          if (p) supportedApis.add(p.api);
        }
        sendJson(res, 200, {
          ok: true,
          target: activeName,
          candidates,
          api: activeProfile.api,
          baseUrl: activeProfile.baseUrl,
          supportedApis: [...supportedApis],
          endpoints: {
            openai: "/v1/chat/completions",
            anthropic: "/v1/messages",
          },
          failover: activeConfig.settings.proxy.failover || [],
          circuitBreaker: activeConfig.settings.proxy.circuitBreaker,
          circuitState: await readCircuitState(),
        });
        return;
      }
      if (url.pathname === "/v1/models") {
        const candidates = buildCandidateNames(activeConfig, options.profile);
        const seen = new Set();
        const data = [];
        for (const candidate of candidates) {
          for (const model of activeConfig.profiles[candidate]?.models || []) {
            if (seen.has(model.id)) continue;
            seen.add(model.id);
            data.push({ id: model.id, object: "model", owned_by: candidate });
          }
        }
        sendJson(res, 200, { object: "list", data });
        return;
      }
      if (url.pathname === "/v1/chat/completions") {
        await forwardWithFailover(req, res, activeConfig, buildCandidateNames(activeConfig, options.profile), "chat/completions");
        return;
      }
      if (url.pathname === "/v1/messages") {
        // Direct Anthropic-compatible endpoint - forward natively
        const candidateNames = buildCandidateNames(activeConfig, options.profile);
        // Filter to anthropic profiles only for direct /v1/messages
        const anthropicCandidates = candidateNames.filter((name) => {
          const p = activeConfig.profiles[name];
          return p && p.api === "anthropic-messages";
        });
        if (anthropicCandidates.length === 0) {
          sendJson(res, 501, { error: { message: "No Anthropic-compatible upstream profile available", type: "unsupported_api" } });
          return;
        }
        await forwardAnthropicWithFailover(req, res, activeConfig, anthropicCandidates);
        return;
      }
      sendJson(res, 404, { error: { message: `Unsupported path ${url.pathname}`, type: "not_found" } });
    } catch (err) {
      sendJson(res, 500, { error: { message: err.message, type: "proxy_error" } });
    }
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, resolve);
  });

  return { server, host, port, target: initialConfig.settings.proxy.target || pickDefaultTarget(initialConfig) };
}
