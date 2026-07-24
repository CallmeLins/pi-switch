import { mkdir, readdir } from "node:fs/promises";
import { join } from "node:path";
import { listPresets, getPreset, presetToProfile } from "./presets.js";
import {
  API_ALIASES,
  CONFIG_DIR,
  CONFIG_PATH,
  MODELS_PATH,
  PI_DIR,
  atomicWriteJson,
  backupFile,
  defaultConfig,
  exists,
  loadConfig,
  normalizeApi,
  parseModel,
  profileToPiProvider,
  providerIdFor,
  readJson,
  saveConfig,
} from "./core.js";

export function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg.startsWith("--")) {
      out._.push(arg);
      continue;
    }
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

export async function init() {
  await mkdir(CONFIG_DIR, { recursive: true });
  await mkdir(PI_DIR, { recursive: true });
  const actions = [];
  if (!(await exists(CONFIG_PATH))) {
    await saveConfig(defaultConfig());
    actions.push(`Created ${CONFIG_PATH}`);
  } else {
    actions.push(`Already exists: ${CONFIG_PATH}`);
  }
  if (!(await exists(MODELS_PATH))) {
    await atomicWriteJson(MODELS_PATH, { providers: {} });
    actions.push(`Created ${MODELS_PATH}`);
  } else {
    actions.push(`Already exists: ${MODELS_PATH}`);
  }
  return actions;
}

export async function add(argv) {
  const args = parseArgs(argv);
  const name = args._[0];
  if (!name) throw new Error("profile name required");

  const presetId = args.preset;
  const preset = presetId ? getPreset(presetId) : null;
  if (presetId && !preset) throw new Error(`unknown preset '${presetId}'`);

  const modelArgs = asArray(args.model);
  const overrides = {
    presetId,
    api: args.api ? normalizeApi(args.api) : undefined,
    baseUrl: args["base-url"] || args.baseUrl,
    apiKey: args["api-key"] || args.apiKey,
    models: modelArgs.map(parseModel),
  };

  let profile;
  if (preset) {
    profile = presetToProfile(preset, overrides);
  } else {
    const api = normalizeApi(args.api);
    const baseUrl = args["base-url"] || args.baseUrl;
    const apiKey = args["api-key"] || args.apiKey;
    if (!baseUrl) throw new Error("--base-url required");
    if (!apiKey) throw new Error("--api-key required. Use literal key or '$ENV_NAME'.");
    if (modelArgs.length === 0) throw new Error("at least one --model required");
    profile = {
      api,
      baseUrl,
      apiKey,
      models: modelArgs.map(parseModel),
      updatedAt: new Date().toISOString()
    };
  }

  if (!profile.baseUrl) throw new Error("--base-url required");
  if (!profile.apiKey) throw new Error("--api-key required. Use literal key or '$ENV_NAME'.");
  if (!Array.isArray(profile.models) || profile.models.length === 0) throw new Error("at least one model required");

  const config = await loadConfig();
  const backup = await backupFile(CONFIG_PATH, "config");
  config.profiles[name] = profile;
  config.current ??= name;
  await saveConfig(config);
  return { name, path: CONFIG_PATH, backup, presetId };
}

export async function list() {
  const config = await loadConfig();
  return { current: config.current, profiles: config.profiles };
}

export async function show(name) {
  const config = await loadConfig();
  const profile = config.profiles[name];
  if (!profile) throw new Error(`unknown profile '${name}'`);
  return { name, profile, providerId: providerIdFor(config, name) };
}

export async function update(name, patch) {
  if (!name) throw new Error("profile name required");
  const config = await loadConfig();
  const profile = config.profiles[name];
  if (!profile) throw new Error(`unknown profile '${name}'`);
  const backup = await backupFile(CONFIG_PATH, "config");
  config.profiles[name] = {
    ...profile,
    ...patch,
    updatedAt: new Date().toISOString(),
  };
  await saveConfig(config);
  return { name, profile: config.profiles[name], backup };
}

export async function remove(name) {
  if (!name) throw new Error("profile name required");
  const config = await loadConfig();
  if (!config.profiles[name]) throw new Error(`unknown profile '${name}'`);
  const backup = await backupFile(CONFIG_PATH, "config");

  // Remove from models.json
  const providerId = providerIdFor(config, name);
  if (await exists(MODELS_PATH)) {
    const models = await readJson(MODELS_PATH, { providers: {} });
    if (models.providers && models.providers[providerId]) {
      delete models.providers[providerId];
      await atomicWriteJson(MODELS_PATH, models);
    }
  }

  delete config.profiles[name];
  if (config.current === name) config.current = Object.keys(config.profiles)[0] || null;
  await saveConfig(config);
  return { name, backup };
}

export async function use(argv) {
  const args = parseArgs(argv);
  const name = args._[0];
  if (!name) throw new Error("profile name required");
  const config = await loadConfig();
  const profile = config.profiles[name];
  if (!profile) throw new Error(`unknown profile '${name}'`);

  const mode = args.mode || config.settings.writeMode || "merge";
  if (!["merge", "exclusive"].includes(mode)) throw new Error("--mode must be merge or exclusive");

  const models = await readJson(MODELS_PATH, { providers: {} });
  models.providers ??= {};
  const providerId = providerIdFor(config, name);

  if (mode === "exclusive") {
    const prefix = `${config.settings.providerPrefix || "pi-switch"}-`;
    for (const key of Object.keys(models.providers)) {
      if (key.startsWith(prefix)) delete models.providers[key];
    }
  }

  models.providers[providerId] = profileToPiProvider(profile, config, name);
  config.current = name;
  await mkdir(PI_DIR, { recursive: true });
  const modelsBackup = await backupFile(MODELS_PATH, "models");
  const configBackup = await backupFile(CONFIG_PATH, "config");
  await atomicWriteJson(MODELS_PATH, models);
  await saveConfig(config);
  return { name, providerId, path: MODELS_PATH, modelsBackup, configBackup };
}

export async function doctor() {
  const checks = [];
  const push = (ok, msg) => checks.push({ ok, msg });
  const isEnvRef = (value) => typeof value === "string" && /^\$\{?[A-Z0-9_]+\}?$/.test(value);
  const envName = (value) => value.replace(/^\$\{?/, "").replace(/\}$/, "");
  const isUrlLike = (value) => {
    try {
      const url = new URL(value);
      return url.protocol === "http:" || url.protocol === "https:";
    } catch {
      return false;
    }
  };

  push(await exists(CONFIG_PATH), `config file: ${CONFIG_PATH}`);
  push(await exists(MODELS_PATH), `pi models file: ${MODELS_PATH}`);

  let config;
  try {
    config = await loadConfig();
    push(true, "config JSON is valid");
  } catch (err) {
    push(false, err.message);
  }

  let models;
  try {
    models = await readJson(MODELS_PATH, { providers: {} });
    push(true, "models.json JSON is valid");
  } catch (err) {
    push(false, err.message);
  }

  if (config) {
    const names = Object.keys(config.profiles || {});
    push(names.length > 0, `${names.length} profile(s) configured`);
    for (const name of names) {
      const p = config.profiles[name];
      push(Boolean(p.baseUrl), `${name}: baseUrl set`);
      push(!p.baseUrl || isUrlLike(p.baseUrl), `${name}: baseUrl is http(s) URL`);
      push(Boolean(p.apiKey), `${name}: apiKey set`);
      if (isEnvRef(p.apiKey)) {
        const key = envName(p.apiKey);
        push(Boolean(process.env[key]), `${name}: env ${key} ${process.env[key] ? "is set" : "is not set"}`);
      }
      push(Boolean(API_ALIASES.has(p.api) || ["openai-completions", "openai-responses", "anthropic-messages", "google-generative-ai"].includes(p.api)), `${name}: api supported (${p.api})`);
      push(Array.isArray(p.models) && p.models.length > 0, `${name}: model list present`);
    }
  }

  if (models) {
    push(typeof models.providers === "object" && !Array.isArray(models.providers), "models.json has providers object");
    if (config?.current && models.providers) {
      const providerId = providerIdFor(config, config.current);
      push(Boolean(models.providers[providerId]), `current provider '${providerId}' is written to models.json`);
    }
  }

  return checks;
}

export async function setProxyTarget(target, failover = undefined) {
  if (!target) throw new Error("target profile required");
  const config = await loadConfig();
  if (!config.profiles[target]) throw new Error(`unknown profile '${target}'`);
  const backup = await backupFile(CONFIG_PATH, "config");
  config.settings.proxy ??= {};
  config.settings.proxy.target = target;
  if (failover !== undefined) {
    for (const name of failover) {
      if (!config.profiles[name]) throw new Error(`unknown failover profile '${name}'`);
      if (config.profiles[name]?.proxy) throw new Error(`failover profile '${name}' is a proxy profile`);
    }
    config.settings.proxy.failover = failover;
  }
  await saveConfig(config);
  return { target, failover: config.settings.proxy.failover || [], backup };
}

export function proxyProviderProfile(host = "127.0.0.1", port = 43112) {
  return {
    api: "openai-completions",
    baseUrl: `http://${host}:${port}/v1`,
    apiKey: "pi-switch",
    proxy: true,
    models: [
      {
        id: "pi-switch-current",
        name: "pi-switch current profile",
        input: ["text"],
        contextWindow: 1000000,
        maxTokens: 128000,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      },
    ],
    updatedAt: new Date().toISOString(),
  };
}

export async function installProxyProvider(argv = []) {
  const args = parseArgs(argv);
  const name = args._[0] || "proxy";
  const host = args.host || "127.0.0.1";
  const port = args.port || 43112;
  const config = await loadConfig();
  const backup = await backupFile(CONFIG_PATH, "config");
  config.profiles[name] = proxyProviderProfile(host, port);
  config.settings.proxy ??= {};
  config.settings.proxy.host = host;
  config.settings.proxy.port = Number(port);
  if (!config.settings.proxy.target) {
    const target = Object.keys(config.profiles).find((profileName) => profileName !== name && !config.profiles[profileName]?.proxy);
    config.settings.proxy.target = target || null;
  }
  config.current = name;
  await saveConfig(config);
  return { name, backup };
}

export async function preset(argv) {
  const args = parseArgs(argv);
  const subcommand = args._[0] || "list";
  if (subcommand === "list") return { type: "list", presets: listPresets() };
  if (subcommand === "show") {
    const id = args._[1];
    if (!id) throw new Error("preset id required");
    const item = getPreset(id);
    if (!item) throw new Error(`unknown preset '${id}'`);
    return { type: "show", id, preset: item };
  }
  throw new Error("usage: pi-switch preset [list|show <id>]");
}

export async function backups() {
  const dir = join(CONFIG_DIR, "backups");
  if (!(await exists(dir))) return [];
  const files = await readdir(dir);
  return files.sort().map((file) => join(dir, file));
}
