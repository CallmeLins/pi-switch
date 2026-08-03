// Type mirror of the Rust config and stats structs in `src-rust/`.
// `src-rust/config.rs` and `src-rust/stats.rs` are the source of truth —
// keep these in sync when they change.
// (Future option noted in WEBUI_GUIDE.md: auto-generate via typeshare/ts-rs.)

export interface ModelCost {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  tiers?: Array<ModelCost & { inputTokensAbove: number }>;
  [key: string]: unknown;
}

export interface ModelEntry {
  id: string;
  name?: string;
  api?: string;
  baseUrl?: string;
  reasoning?: boolean;
  thinkingLevelMap?: Record<string, string | null>;
  input: string[];
  contextWindow: number;
  maxTokens: number;
  cost?: ModelCost;
  headers?: Record<string, string>;
  compat?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface ProviderProfile {
  name?: string;
  api: string;
  baseUrl: string;
  apiKey: string;
  models: ModelEntry[];
  oauth?: "radius";
  preset?: string;
  headers?: Record<string, string>;
  authHeader?: boolean;
  compat?: Record<string, unknown>;
  modelOverrides?: Record<string, Record<string, unknown>>;
  proxy: boolean;
  updatedAt?: string;
  modelMap?: Record<string, unknown>;
  exposedModels?: string[];
  userAgent?: string;
  [key: string]: unknown;
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

export interface TokenTotals {
  input: number;
  output: number;
  total: number;
  cached: number;
  reasoning: number;
}

export interface ProviderStats {
  total: number;
  ok: number;
  failed: number;
  retries: number;
  avgMs: number;
  totalMs: number;
  lastUsed?: string;
  promptTokens: number;
  outputTokens: number;
  cachedTokens: number;
  reasoningTokens: number;
}

export interface ConversationStats {
  conversationId: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  cachedTokens: number;
  reasoningTokens: number;
  lastActive?: string;
  cacheRate?: string;
  cost?: number | null;
}

export interface RecentRequest {
  ts?: string | null;
  provider?: string | null;
  model?: string | null;
  ok?: boolean | null;
  status?: number | null;
  error?: string | null;
  promptTokens?: number | null;
  completionTokens?: number | null;
  cachedTokens?: number | null;
  reasoningTokens?: number | null;
  totalTokens?: number | null;
  cacheRate?: string;
  cost?: number | null;
}

export interface UsageStats {
  totalRequests: number;
  okRequests: number;
  failedRequests: number;
  successRate: string;
  avgLatencyMs?: number;
  byProvider: Record<string, ProviderStats>;
  byModel?: Record<string, { total: number; ok: number; failed: number }>;
  totalTokens?: TokenTotals;
  cacheHitRate?: string;
  totalCost?: number | null;
  costUnknown?: number;
  byConversation?: ConversationStats[];
  recentRequests?: RecentRequest[];
  [key: string]: unknown;
}

export interface PackageEntry {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
  installedAt?: string;
  hasExtensions?: boolean;
  hasSkills?: boolean;
  hasPrompts?: boolean;
  hasThemes?: boolean;
}

export interface CcsProvider {
  id: string;
  name: string;
  appType: string;
  api: string;
  baseUrl: string;
  apiKey: string;
  models: string[];
  exists: boolean;
}

export interface CcsImportResult {
  name: string;
  imported: boolean;
  message: string;
}
