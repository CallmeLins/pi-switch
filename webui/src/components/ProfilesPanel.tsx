import { useEffect, useMemo, useState } from "react";
import type { AppState, CcsProvider, ModelEntry, PresetInfo, ProviderProfile, ResponsesMode } from "../types";
import { effectiveResponsesMode, responsesModeError } from "../lib/responsesMode";
import { api } from "../api";
import { useI18n } from "../i18n";
import {
  Badge,
  Button,
  Card,
  Field,
  Input,
  Modal,
  SectionTitle,
  Select,
  Textarea,
  useAction,
  cx,
} from "./ui";

const API_TYPES = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
];
const SPOOFS = [
  { value: "", label: "none" },
  { value: "claude-code", label: "claude-code" },
  { value: "codex", label: "codex" },
  { value: "gemini", label: "gemini" },
];

function defaultModel(id: string): ModelEntry {
  return { id, input: ["text"], contextWindow: 128000, maxTokens: 16384 };
}

export function ProfilesPanel({
  state,
  refresh,
}: {
  state: AppState;
  refresh: () => Promise<void>;
}) {
  const run = useAction();
  const { t } = useI18n();
  const [editing, setEditing] = useState<{ name: string | null } | null>(null);
  const [models, setModels] = useState<string | null>(null); // profile name for models modal
  const [ccImport, setCcImport] = useState(false);

  const entries = Object.entries(state.profiles).sort(([a], [b]) => a.localeCompare(b));

  return (
    <div>
      <SectionTitle hint={`${entries.length} ${t("profile(s)")}`}>{t("Profiles")}</SectionTitle>

      <div className="mb-3 flex gap-2">
        <Button variant="primary" onClick={() => setEditing({ name: null })}>
          {t("+ Add profile")}
        </Button>
        <Button onClick={() => setCcImport(true)}>⇥ {t("Import from cc-switch")}</Button>
      </div>

      <div className="space-y-2">
        {entries.length === 0 && (
          <Card>
            <div className="text-sm text-zinc-500">
              {t("No profiles yet.")} {t('Add one with the "+ Add profile" button or import from cc-switch.')}
            </div>
          </Card>
        )}
        {entries.map(([name, p]) => {
          const isCurrent = state.current === name;
          const exposed = p.exposedModels?.length ?? 0;
          return (
            <Card key={name} className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="truncate font-medium text-zinc-100">{name}</span>
                  {isCurrent && <Badge tone="indigo">{t("current")}</Badge>}
                  {p.proxy && <Badge tone="amber">{t("proxy")}</Badge>}
                  <Badge>{p.api || "?"}</Badge>
                  <Badge tone="indigo">{t("Responses")}: {effectiveResponsesMode(p)}</Badge>
                  {exposed > 0 && <Badge tone="green">{exposed} {t("exposed")}</Badge>}
                </div>
                <div className="mt-0.5 truncate text-xs text-zinc-500">
                  {p.baseUrl || t("no base url")} · {p.models?.length ?? 0} {t("models")}
                </div>
              </div>
              <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
                {!isCurrent && (
                  <Button
                    onClick={() =>
                      run(() => api.useProfile(name), `${t("Switched to")} ${name}`, refresh)
                    }
                  >
                    {t("Use")}
                  </Button>
                )}
                <Button onClick={() => setModels(name)}>{t("Models")}</Button>
                <Button onClick={() => setEditing({ name })}>{t("Edit")}</Button>
                <Button
                  onClick={() =>
                    run(
                      async () => {
                        const r = await api.testProfile(name);
                        if (!r.success) throw new Error(r.message);
                        return r;
                      },
                      t("Test OK"),
                    )
                  }
                >
                  {t("Test")}
                </Button>
                <Button
                  onClick={() => {
                    const to = prompt(
                      `${t("Duplicate profile '{{name}}' as:").replace("{{name}}", name)}`,
                      `${name}-copy`,
                    );
                    if (to) run(() => api.duplicateProfile(name, to), t("Duplicated"), refresh);
                  }}
                >
                  {t("Copy")}
                </Button>
                <Button
                  variant="danger"
                  onClick={() => {
                    if (confirm(t("Delete profile '{{name}}'?").replace("{{name}}", name)))
                      run(() => api.deleteProfile(name), t("Deleted"), refresh);
                  }}
                >
                  {t("Delete")}
                </Button>
              </div>
            </Card>
          );
        })}
      </div>

      {editing && (
        <ProfileForm
          state={state}
          original={editing.name}
          onClose={() => setEditing(null)}
          onSaved={async () => {
            setEditing(null);
            await refresh();
          }}
        />
      )}

      {models && (
        <ModelsModal
          name={models}
          profile={state.profiles[models]}
          onClose={() => setModels(null)}
          onSaved={async () => {
            setModels(null);
            await refresh();
          }}
        />
      )}

      {ccImport && (
        <CcsImportModal
          onClose={() => setCcImport(false)}
          onImported={async () => {
            setCcImport(false);
            await refresh();
          }}
        />
      )}
    </div>
  );
}

// ─── Add / Edit form ──────────────────────────────────────

function ProfileForm({
  state,
  original,
  onClose,
  onSaved,
}: {
  state: AppState;
  original: string | null;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const run = useAction();
  const { t } = useI18n();
  const existing = original ? state.profiles[original] : undefined;
  const presets = usePresets();

  const [name, setName] = useState(original ?? "");
  const [apiType, setApiType] = useState(existing?.api ?? "openai-completions");
  const [responsesMode, setResponsesMode] = useState<ResponsesMode>(existing?.responsesMode ?? "auto");
  const [baseUrl, setBaseUrl] = useState(existing?.baseUrl ?? "");
  const [apiKey, setApiKey] = useState(existing?.apiKey ?? "");
  const [spoof, setSpoof] = useState(existing?.userAgent ?? "");
  const [proxy, setProxy] = useState(existing?.proxy ?? false);
  const [preset, setPreset] = useState(existing?.preset ?? "");
  const [modelIds, setModelIds] = useState(
    (existing?.models ?? []).map((m) => m.id).join("\n"),
  );

  function applyPreset(id: string) {
    setPreset(id);
    const p = presets.find((x) => x.id === id);
    if (!p) return;
    setApiType(p.api);
    setBaseUrl(p.baseUrl);
    if (!modelIds.trim()) setModelIds(p.models.join("\n"));
  }

  function build(): ProviderProfile {
    const ids = modelIds
      .split(/[\n,]/)
      .map((s) => s.trim())
      .filter(Boolean);
    // Preserve existing model metadata by id; default for new ids.
    const prevById = new Map((existing?.models ?? []).map((m) => [m.id, m]));
    const models = ids.map((id) => prevById.get(id) ?? defaultModel(id));
    const exposedModels = (existing?.exposedModels ?? []).filter((id) => ids.includes(id));
    return {
      ...(existing ?? {}),
      api: apiType,
      responsesMode,
      baseUrl: baseUrl.trim(),
      apiKey: apiKey.trim(),
      models,
      proxy,
      exposedModels,
      preset: preset || undefined,
      userAgent: spoof || undefined,
      updatedAt: new Date().toISOString(),
    } as ProviderProfile;
  }

  async function save() {
    const trimmed = name.trim();
    if (!trimmed) throw new Error(t("name required"));
    const modeError = responsesModeError(apiType, responsesMode);
    if (modeError) throw new Error(t(modeError));
    const profile = build();
    if (original) {
      await api.updateProfile(trimmed, profile, original !== trimmed ? original : undefined);
    } else {
      await api.addProfile(trimmed, profile);
    }
  }

  return (
    <Modal title={original ? `${t("Edit")} ${original}` : t("Add profile")} onClose={onClose} wide>
      <div className="grid gap-x-4 sm:grid-cols-2">
        <Field label={t("Name")}>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="my-provider" />
        </Field>
        <Field label={t("Preset (prefill)")}>
          <Select value={preset} onChange={(e) => applyPreset(e.target.value)}>
            <option value="">— {t("none")} —</option>
            {presets.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </Select>
        </Field>
        <Field label={t("API type")}>
          <Select value={apiType} onChange={(e) => setApiType(e.target.value)}>
            {API_TYPES.map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </Select>
        </Field>
        <Field label={t("Responses mode")}>
          <Select value={responsesMode} onChange={(e) => setResponsesMode(e.target.value as ResponsesMode)}>
            <option value="auto">auto — {t("automatic by API type")}</option>
            <option value="passthrough">passthrough — {t("native Responses only")}</option>
            <option value="convert">convert — {t("Chat Completions only")}</option>
          </Select>
        </Field>
        <Field label={t("Disguise (User-Agent)")}>
          <Select value={spoof} onChange={(e) => setSpoof(e.target.value)}>
            {SPOOFS.map((s) => (
              <option key={s.value} value={s.value}>
                {s.label}
              </option>
            ))}
          </Select>
        </Field>
        <div className="sm:col-span-2">
          <Field label={t("Base URL")}>
            <Input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.example.com/v1"
            />
          </Field>
        </div>
        <div className="sm:col-span-2">
          <Field label={t("API key (supports $ENV_VAR)")}>
            <Input value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" />
          </Field>
        </div>
        <div className="sm:col-span-2">
          <Field label={t("Model IDs (one per line)")}>
            <Textarea
              rows={4}
              value={modelIds}
              onChange={(e) => setModelIds(e.target.value)}
              placeholder={"gpt-4o\ngpt-4o-mini"}
            />
          </Field>
        </div>
        <label className="mb-3 flex items-center gap-2 text-sm text-zinc-300 sm:col-span-2">
          <input type="checkbox" checked={proxy} onChange={(e) => setProxy(e.target.checked)} />
          {t("Mark as a proxy profile (excluded from failover, not exposed to pi)")}
        </label>
      </div>

      <div className="mt-2 flex justify-end gap-2">
        <Button onClick={onClose}>{t("Cancel")}</Button>
        <Button variant="primary" onClick={() => run(save, t("Saved"), onSaved)}>
          {t("Save")}
        </Button>
      </div>
    </Modal>
  );
}

// ─── Models & expose modal ────────────────────────────────

function ModelsModal({
  name,
  profile,
  onClose,
  onSaved,
}: {
  name: string;
  profile: ProviderProfile;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const run = useAction();
  const { t } = useI18n();
  const [models, setModels] = useState<ModelEntry[]>(profile.models ?? []);
  const [exposed, setExposed] = useState<Set<string>>(
    new Set(profile.exposedModels ?? []),
  );
  const [newId, setNewId] = useState("");
  const [fetching, setFetching] = useState(false);

  function toggle(id: string) {
    setExposed((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  }

  function addId(id: string) {
    const trimmed = id.trim();
    if (!trimmed || models.some((m) => m.id === trimmed)) return;
    setModels((m) => [...m, defaultModel(trimmed)]);
  }

  async function fetchFromProvider() {
    setFetching(true);
    try {
      const { models: ids } = await api.fetchModels(name);
      setModels((prev) => {
        const have = new Set(prev.map((m) => m.id));
        const added = ids.filter((id) => !have.has(id)).map(defaultModel);
        return [...prev, ...added];
      });
    } finally {
      setFetching(false);
    }
  }

  async function save() {
    await api.updateModels(name, models);
    await api.expose(name, [...exposed].filter((id) => models.some((m) => m.id === id)));
  }

  return (
    <Modal title={`${t("Models")} · ${name}`} onClose={onClose} wide>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <Input
          value={newId}
          onChange={(e) => setNewId(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              addId(newId);
              setNewId("");
            }
          }}
          placeholder={t("add model id + Enter")}
          className="max-w-xs"
        />
        <Button
          onClick={() => run(fetchFromProvider, undefined)}
          disabled={fetching}
        >
          {fetching ? t("Fetching…") : t("Fetch from provider")}
        </Button>
        <span className="text-xs text-zinc-500">
          {t("Checked = exposed to pi as")} <code>{name}/&lt;id&gt;</code>
        </span>
      </div>

      <div className="max-h-80 space-y-1 overflow-y-auto rounded-lg border border-white/10 p-2">
        {models.length === 0 && (
          <div className="p-3 text-sm text-zinc-500">
            {t("No models. Add ids above or fetch from the provider.")}
          </div>
        )}
        {models.map((m) => (
          <div
            key={m.id}
            className={cx(
              "flex items-center justify-between rounded-md px-2 py-1.5 text-sm",
              exposed.has(m.id) ? "bg-emerald-500/10" : "hover:bg-white/5",
            )}
          >
            <label className="flex min-w-0 items-center gap-2">
              <input
                type="checkbox"
                checked={exposed.has(m.id)}
                onChange={() => toggle(m.id)}
              />
              <span className="truncate text-zinc-200">{m.id}</span>
            </label>
            <button
              className="text-xs text-zinc-500 hover:text-red-300"
              onClick={() => {
                setModels((prev) => prev.filter((x) => x.id !== m.id));
                setExposed((prev) => {
                  const n = new Set(prev);
                  n.delete(m.id);
                  return n;
                });
              }}
            >
              {t("remove")}
            </button>
          </div>
        ))}
      </div>

      <div className="mt-4 flex items-center justify-between">
        <div className="flex gap-2 text-xs">
          <button
            className="text-zinc-400 hover:text-zinc-200"
            onClick={() => setExposed(new Set(models.map((m) => m.id)))}
          >
            {t("expose all")}
          </button>
          <button
            className="text-zinc-400 hover:text-zinc-200"
            onClick={() => setExposed(new Set())}
          >
            {t("expose none")}
          </button>
        </div>
        <div className="flex gap-2">
          <Button onClick={onClose}>{t("Cancel")}</Button>
          <Button variant="primary" onClick={() => run(save, t("Models saved"), onSaved)}>
            {t("Save")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

// ─── preset loader ────────────────────────────────────────

let presetCache: PresetInfo[] | null = null;
function usePresets(): PresetInfo[] {
  const [presets, setPresets] = useState<PresetInfo[]>(presetCache ?? []);
  useMemo(() => {
    if (presetCache) return;
    api
      .getPresets()
      .then((p) => {
        presetCache = p;
        setPresets(p);
      })
      .catch(() => {});
  }, []);
  return presets;
}

// ─── cc-switch import modal ───────────────────────────────

function CcsImportModal({
  onClose,
  onImported,
}: {
  onClose: () => void;
  onImported: () => Promise<void>;
}) {
  const run = useAction();
  const { t } = useI18n();
  const [providers, setProviders] = useState<CcsProvider[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [path, setPath] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const load = async (p?: string) => {
    setError(null);
    setProviders(null);
    try {
      const data = await api.ccsProviders(p || undefined);
      setProviders(data.providers);
      setSelected(new Set(data.providers.filter((x) => !x.exists).map((x) => x.id)));
    } catch (err) {
      setError((err as Error).message);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const doImport = async () => {
    const selections = [...selected].map((id) => ({ id }));
    if (selections.length === 0) return;
    const data = await api.importCcs(selections, path || undefined);
    if (data.imported > 0) {
      alert(t("Imported {{n}} provider(s) from cc-switch").replace("{{n}}", String(data.imported)));
      await onImported();
    } else {
      alert(t("Nothing imported (already exist or skipped)."));
    }
  };

  const apiLabel = (api: string) =>
    ({ "anthropic-messages": "anthropic", "openai-responses": "openai", "google-generative-ai": "gemini" })[api] ?? api;

  return (
    <Modal title={t("Import from cc-switch")} onClose={onClose} wide>
      <div className="space-y-3">
        {error && (
          <div>
            <p style={{ color: "#ff5555", fontSize: "0.9rem" }}>{error}</p>
            <div className="flex gap-2 mt-2">
              <Input
                placeholder={t("Path to cc-switch.db (optional)")}
                value={path}
                onChange={(e) => setPath(e.target.value)}
              />
              <Button onClick={() => void load(path)}>{t("Retry")}</Button>
            </div>
          </div>
        )}

        {providers === null && !error && <p style={{ color: "#999" }}>{t("Loading…")}</p>}

        {providers && providers.length === 0 && (
          <p style={{ color: "#999" }}>{t("No importable providers found in cc-switch.")}</p>
        )}

        {providers && providers.length > 0 && (
          <div className="space-y-2 max-h-80 overflow-auto rounded-lg border border-white/10 p-2">
            {providers.map((p) => (
              <label
                key={p.id}
                className={
                  "flex items-start gap-2 p-2 rounded-md cursor-pointer transition-colors " +
                  (selected.has(p.id)
                    ? "bg-emerald-500/15 border border-emerald-500/30"
                    : "bg-white/5 border border-transparent hover:bg-white/10")
                }
              >
                <input
                  type="checkbox"
                  checked={selected.has(p.id)}
                  onChange={() => toggle(p.id)}
                  className="mt-0.5 accent-emerald-500"
                />
                <span className="text-sm min-w-0">
                  <span className="font-medium text-zinc-100">{p.name}</span>{" "}
                  <Badge>{apiLabel(p.api)}</Badge>{" "}
                  {p.exists && <Badge>{t("exists")}</Badge>}
                  <br />
                  <span className="text-zinc-500 break-all">{p.baseUrl}</span>
                  <br />
                  <span className="text-zinc-400">{p.models.join(", ") || "-"}</span>
                </span>
              </label>
            ))}
          </div>
        )}

        <div className="flex gap-2 justify-end">
          <Button onClick={onClose}>{t("Cancel")}</Button>
          <Button variant="primary" disabled={!providers || selected.size === 0} onClick={() => void doImport()}>
            {t("Import selected")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
