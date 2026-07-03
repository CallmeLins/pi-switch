// Type mirror of the Rust config structs in `src-rust/config.rs`.
// That file is the source of truth — keep these in sync when it changes.
// (Future option noted in WEBUI_GUIDE.md: auto-generate via typeshare/ts-rs.)

export interface ModelCost {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
}

export interface ModelEntry {
  id: string;
  name?: string;
  input: string[];
  contextWindow: number;
  maxTokens: number;
  cost?: ModelCost;
}

export interface ProviderProfile {
  api: string;
  baseUrl: string;
  apiKey: string;
  models: ModelEntry[];
  preset?: string;
  headers?: Record<string, unknown>;
  authHeader?: string;
  compat?: string;
  proxy: boolean;
  updatedAt?: string;
  modelMap?: Record<string, unknown>;
  exposedModels?: string[];
  userAgent?: string;
}

export interface CircuitBreakerSettings {
  enabled: boolean;
  failureThreshold: number;
  cooldownSeconds: number;
}

export interface ProxySettings {
  host: string;
  port: number;
  target?: string;
  failover: string[];
  userAgent?: string;
  circuitBreaker: CircuitBreakerSettings;
}

export interface WebSettings {
  host: string;
  port: number;
}

export interface Settings {
  providerPrefix: string;
  writeMode: string;
  language?: string | null;
  proxy: ProxySettings;
  web: WebSettings;
}

export interface AppState {
  current?: string | null;
  profiles: Record<string, ProviderProfile>;
  settings: Settings;
}

export interface PresetInfo {
  id: string;
  name: string;
  description: string;
  websiteUrl: string;
  api: string;
  baseUrl: string;
  models: string[];
}

export interface DoctorCheck {
  ok: boolean;
  msg: string;
}

export interface ValidationIssue {
  level: string;
  path: string;
  message: string;
}

export interface DaemonResult {
  running: boolean;
  pid?: number;
  host?: string;
  port?: number;
  targets?: string[];
  failover?: string[];
  startedAt?: number;
  message: string;
}

export interface TestResult {
  success: boolean;
  message: string;
  responseTimeMs?: number;
}

export interface ProfileDetail {
  name: string;
  profile: ProviderProfile;
  providerId: string;
}

export interface UsageStats {
  totalRequests: number;
  okRequests: number;
  failedRequests: number;
  successRate: string;
  avgLatencyMs?: number;
  byProvider: Record<string, { total: number; ok: number; failed: number }>;
  byModel?: Record<string, { total: number; ok: number; failed: number }>;
  [key: string]: unknown;
}
