import { access, copyFile, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import { dirname, join } from "node:path";
import { homedir } from "node:os";

export const CONFIG_DIR = join(homedir(), ".pi-switch");
export const CONFIG_PATH = join(CONFIG_DIR, "config.json");
export const PI_DIR = join(homedir(), ".pi", "agent");
export const MODELS_PATH = join(PI_DIR, "models.json");
export const BACKUP_DIR = join(CONFIG_DIR, "backups");

export const API_ALIASES = new Map([
  ["openai", "openai-completions"],
  ["openai-completions", "openai-completions"],
  ["anthropic", "anthropic-messages"],
  ["anthropic-messages", "anthropic-messages"],
  ["google", "google-generative-ai"],
  ["google-generative-ai", "google-generative-ai"],
]);

export function timestamp() {
  return new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
}

export async function exists(path) {
  try {
    await access(path, constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

export async function readJson(path, fallback) {
  if (!(await exists(path))) return fallback;
  const text = await readFile(path, "utf8");
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error(`${path} is not valid JSON: ${err.message}`);
  }
}

export async function atomicWriteJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  const tmp = `${path}.tmp-${process.pid}-${Date.now()}`;
  await writeFile(tmp, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  await rename(tmp, path);
}

export async function backupFile(path, label) {
  if (!(await exists(path))) return null;
  await mkdir(BACKUP_DIR, { recursive: true });
  const backupPath = join(BACKUP_DIR, `${label}-${timestamp()}.json`);
  await copyFile(path, backupPath);
  return backupPath;
}

export function defaultConfig() {
  return {
    version: 1,
    current: null,
    profiles: {},
    settings: {
      providerPrefix: "pi-switch",
      writeMode: "merge",
      proxy: {
        host: "127.0.0.1",
        port: 43112,
        target: null,
        failover: [],
        circuitBreaker: {
          enabled: true,
          failureThreshold: 3,
          cooldownSeconds: 60
        }
      }
    }
  };
}

export async function loadConfig() {
  const config = await readJson(CONFIG_PATH, defaultConfig());
  config.version ??= 1;
  config.current ??= null;
  config.profiles ??= {};
  config.settings ??= {};
  config.settings.providerPrefix ??= "pi-switch";
  config.settings.writeMode ??= "merge";
  config.settings.proxy ??= {};
  config.settings.proxy.host ??= "127.0.0.1";
  config.settings.proxy.port ??= 43112;
  config.settings.proxy.target ??= null;
  config.settings.proxy.failover ??= [];
  config.settings.proxy.circuitBreaker ??= {};
  config.settings.proxy.circuitBreaker.enabled ??= true;
  config.settings.proxy.circuitBreaker.failureThreshold ??= 3;
  config.settings.proxy.circuitBreaker.cooldownSeconds ??= 60;
  return config;
}

export async function saveConfig(config) {
  await atomicWriteJson(CONFIG_PATH, config);
}

export function normalizeApi(api) {
  const normalized = API_ALIASES.get(String(api || "").trim());
  if (!normalized) throw new Error(`unsupported api '${api}'. Use openai, anthropic, or the full pi api id.`);
  return normalized;
}

export function parseModel(modelArg) {
  const idx = modelArg.indexOf("=");
  const id = idx === -1 ? modelArg.trim() : modelArg.slice(0, idx).trim();
  const name = idx === -1 ? undefined : modelArg.slice(idx + 1).trim();
  if (!id) throw new Error(`invalid --model '${modelArg}'`);
  return {
    id,
    ...(name ? { name } : {}),
    input: ["text"],
    contextWindow: 128000,
    maxTokens: 16384,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }
  };
}

export function providerIdFor(config, profileName) {
  const prefix = config.settings.providerPrefix || "pi-switch";
  return `${prefix}-${profileName}`;
}

export function profileToPiProvider(profile) {
  return {
    baseUrl: profile.baseUrl,
    api: profile.api,
    apiKey: profile.apiKey,
    ...(profile.headers && Object.keys(profile.headers).length ? { headers: profile.headers } : {}),
    ...(profile.authHeader !== undefined ? { authHeader: profile.authHeader } : {}),
    ...(profile.compat ? { compat: profile.compat } : {}),
    models: profile.models
  };
}
