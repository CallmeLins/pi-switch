import type {
  AppState,
  DaemonResult,
  DoctorCheck,
  ModelEntry,
  PresetInfo,
  ProfileDetail,
  ProviderProfile,
  TestResult,
  UsageStats,
  ValidationIssue,
} from "./types";

// Single point of coupling to the backend. Every call maps to one REST route in
// src-rust/web.rs, which in turn delegates to the shared ops/service layer.
async function req<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method,
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) {
    throw new Error((data && data.error) || res.statusText || "request failed");
  }
  return data as T;
}

const enc = encodeURIComponent;

export const api = {
  // reads
  getState: () => req<AppState>("GET", "/state"),
  getPresets: () => req<PresetInfo[]>("GET", "/presets"),
  getPreset: (id: string) => req<ProviderProfile & { name?: string }>("GET", `/presets/${enc(id)}`),
  getProfile: (name: string) => req<ProfileDetail>("GET", `/profiles/${enc(name)}`),
  doctor: () => req<DoctorCheck[]>("GET", "/doctor"),
  validate: () => req<ValidationIssue[]>("GET", "/config/validate"),
  backups: () => req<string[]>("GET", "/backups"),
  stats: () => req<UsageStats>("GET", "/stats"),
  proxyStatus: () => req<DaemonResult>("GET", "/proxy/status"),
  webuiInfo: () => req<{ authRequired: boolean }>("GET", "/webui/info"),

  // profile mutations
  init: () => req<{ messages: string[] }>("POST", "/init"),
  addProfile: (name: string, profile: ProviderProfile) =>
    req("POST", "/profiles", { name, profile }),
  updateProfile: (name: string, profile: ProviderProfile, renameFrom?: string) =>
    req("PUT", `/profiles/${enc(name)}`, { profile, renameFrom }),
  deleteProfile: (name: string) => req("DELETE", `/profiles/${enc(name)}`),
  duplicateProfile: (name: string, asName: string) =>
    req("POST", `/profiles/${enc(name)}/duplicate`, { as: asName }),
  useProfile: (name: string, mode?: string) =>
    req("POST", `/profiles/${enc(name)}/use`, mode ? { mode } : {}),
  testProfile: (name: string) =>
    req<TestResult>("POST", `/profiles/${enc(name)}/test`),
  fetchModels: (name: string) =>
    req<{ models: string[] }>("POST", `/profiles/${enc(name)}/fetch-models`),
  updateModels: (name: string, models: ModelEntry[]) =>
    req("PUT", `/profiles/${enc(name)}/models`, { models }),
  expose: (name: string, modelIds: string[]) =>
    req("PUT", `/profiles/${enc(name)}/expose`, { modelIds }),
  setSpoof: (name: string, spoof: string | null) =>
    req("PUT", `/profiles/${enc(name)}/spoof`, { spoof }),

  // proxy + settings + config
  proxyStart: (host?: string, port?: number) =>
    req<DaemonResult>("POST", "/proxy/start", { host, port }),
  proxyStop: () => req<DaemonResult>("POST", "/proxy/stop"),
  setFailover: (failover: string[]) => req("PUT", "/proxy/failover", { failover }),
  updateSettings: (settings: AppState["settings"]) => req("PUT", "/settings", settings),
  exportConfig: (passphrase: string) =>
    req<{ path: string }>("POST", "/config/export", { passphrase }),
  importConfig: (filePath: string, passphrase: string) =>
    req<{ message: string }>("POST", "/config/import", { filePath, passphrase }),
  restoreConfig: (backupPath: string) =>
    req<{ backup: string }>("POST", "/config/restore", { backupPath }),
};

export function logsExportUrl(format: "json" | "csv"): string {
  return `/api/logs/export?format=${format}`;
}
