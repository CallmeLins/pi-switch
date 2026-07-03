import { useMemo, useState } from "react";
import type { AppState, ModelEntry, PresetInfo, ProviderProfile } from "../types";
import { api } from "../api";
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

const API_TYPES = ["openai-completions", "anthropic-messages", "google-generative-ai"];
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
  const [editing, setEditing] = useState<{ name: string | null } | null>(null);
  const [models, setModels] = useState<string | null>(null); // profile name for models modal

  const entries = Object.entries(state.profiles).sort(([a], [b]) => a.localeCompare(b));

  return (
    <div>
      <SectionTitle hint={`${entries.length} profile(s)`}>Profiles</SectionTitle>

      <div className="mb-3">
        <Button variant="primary" onClick={() => setEditing({ name: null })}>
          + Add profile
        </Button>
      </div>

      <div className="space-y-2">
        {entries.length === 0 && (
          <Card>
            <div className="text-sm text-zinc-500">No profiles yet. Add one to get started.</div>
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
                  {isCurrent && <Badge tone="indigo">current</Badge>}
                  {p.proxy && <Badge tone="amber">proxy</Badge>}
                  <Badge>{p.api || "?"}</Badge>
                  {exposed > 0 && <Badge tone="green">{exposed} exposed</Badge>}
                </div>
                <div className="mt-0.5 truncate text-xs text-zinc-500">
                  {p.baseUrl || "no base url"} · {p.models?.length ?? 0} models
                </div>
              </div>
              <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
                {!isCurrent && (
                  <Button
                    onClick={() =>
                      run(() => api.useProfile(name), `Switched to ${name}`, refresh)
                    }
                  >
                    Use
                  </Button>
                )}
                <Button onClick={() => setModels(name)}>Models</Button>
                <Button onClick={() => setEditing({ name })}>Edit</Button>
                <Button
                  onClick={() =>
                    run(
                      async () => {
                        const r = await api.testProfile(name);
                        if (!r.success) throw new Error(r.message);
                        return r;
                      },
                      `Test OK`,
                    )
                  }
                >
                  Test
                </Button>
                <Button
                  onClick={() => {
                    const to = prompt(`Duplicate '${name}' as:`, `${name}-copy`);
                    if (to) run(() => api.duplicateProfile(name, to), "Duplicated", refresh);
                  }}
                >
                  Copy
                </Button>
                <Button
                  variant="danger"
                  onClick={() => {
                    if (confirm(`Delete profile '${name}'?`))
                      run(() => api.deleteProfile(name), "Deleted", refresh);
                  }}
                >
                  Delete
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
  const existing = original ? state.profiles[original] : undefined;
  const presets = usePresets();

  const [name, setName] = useState(original ?? "");
  const [apiType, setApiType] = useState(existing?.api ?? "openai-completions");
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
    if (!trimmed) throw new Error("name required");
    const profile = build();
    if (original) {
      await api.updateProfile(trimmed, profile, original !== trimmed ? original : undefined);
    } else {
      await api.addProfile(trimmed, profile);
    }
  }

  return (
    <Modal title={original ? `Edit ${original}` : "Add profile"} onClose={onClose} wide>
      <div className="grid gap-x-4 sm:grid-cols-2">
        <Field label="Name">
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="my-provider" />
        </Field>
        <Field label="Preset (prefill)">
          <Select value={preset} onChange={(e) => applyPreset(e.target.value)}>
            <option value="">— none —</option>
            {presets.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="API type">
          <Select value={apiType} onChange={(e) => setApiType(e.target.value)}>
            {API_TYPES.map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="Disguise (User-Agent)">
          <Select value={spoof} onChange={(e) => setSpoof(e.target.value)}>
            {SPOOFS.map((s) => (
              <option key={s.value} value={s.value}>
                {s.label}
              </option>
            ))}
          </Select>
        </Field>
        <div className="sm:col-span-2">
          <Field label="Base URL">
            <Input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.example.com/v1"
            />
          </Field>
        </div>
        <div className="sm:col-span-2">
          <Field label="API key (supports $ENV_VAR)">
            <Input value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" />
          </Field>
        </div>
        <div className="sm:col-span-2">
          <Field label="Model IDs (one per line)">
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
          Mark as a proxy profile (excluded from failover, not exposed to pi)
        </label>
      </div>

      <div className="mt-2 flex justify-end gap-2">
        <Button onClick={onClose}>Cancel</Button>
        <Button variant="primary" onClick={() => run(save, "Saved", onSaved)}>
          Save
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
    <Modal title={`Models · ${name}`} onClose={onClose} wide>
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
          placeholder="add model id + Enter"
          className="max-w-xs"
        />
        <Button
          onClick={() => run(fetchFromProvider, undefined)}
          disabled={fetching}
        >
          {fetching ? "Fetching…" : "Fetch from provider"}
        </Button>
        <span className="text-xs text-zinc-500">
          Checked = exposed to pi as <code>{name}/&lt;id&gt;</code>
        </span>
      </div>

      <div className="max-h-80 space-y-1 overflow-y-auto rounded-lg border border-white/10 p-2">
        {models.length === 0 && (
          <div className="p-3 text-sm text-zinc-500">
            No models. Add ids above or fetch from the provider.
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
              remove
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
            expose all
          </button>
          <button
            className="text-zinc-400 hover:text-zinc-200"
            onClick={() => setExposed(new Set())}
          >
            expose none
          </button>
        </div>
        <div className="flex gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => run(save, "Models saved", onSaved)}>
            Save
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
