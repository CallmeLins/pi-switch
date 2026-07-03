import { useState } from "react";
import type { AppState, Settings } from "../types";
import { api } from "../api";
import { Button, Card, Field, Input, SectionTitle, Select, useAction } from "./ui";

export function SettingsPanel({
  state,
  refresh,
}: {
  state: AppState;
  refresh: () => Promise<void>;
}) {
  const run = useAction();
  // Deep clone so edits don't mutate the shared state until saved.
  const [s, setS] = useState<Settings>(() => JSON.parse(JSON.stringify(state.settings)));

  const set = (patch: Partial<Settings>) => setS((prev) => ({ ...prev, ...patch }));
  const setProxy = (patch: Partial<Settings["proxy"]>) =>
    setS((prev) => ({ ...prev, proxy: { ...prev.proxy, ...patch } }));
  const setCb = (patch: Partial<Settings["proxy"]["circuitBreaker"]>) =>
    setS((prev) => ({
      ...prev,
      proxy: { ...prev.proxy, circuitBreaker: { ...prev.proxy.circuitBreaker, ...patch } },
    }));
  const setWeb = (patch: Partial<Settings["web"]>) =>
    setS((prev) => ({ ...prev, web: { ...prev.web, ...patch } }));

  return (
    <div>
      <SectionTitle hint="written to ~/.pi-switch/config.json">Settings</SectionTitle>

      <Card className="mb-4">
        <div className="mb-3 text-sm font-semibold text-zinc-200">General</div>
        <div className="grid gap-x-4 sm:grid-cols-2">
          <Field label="Provider prefix (pi gateway id)">
            <Input
              value={s.providerPrefix}
              onChange={(e) => set({ providerPrefix: e.target.value })}
            />
          </Field>
          <Field label="Write mode">
            <Select value={s.writeMode} onChange={(e) => set({ writeMode: e.target.value })}>
              <option value="merge">merge</option>
              <option value="exclusive">exclusive</option>
            </Select>
          </Field>
          <Field label="Language">
            <Select
              value={s.language ?? ""}
              onChange={(e) => set({ language: e.target.value || null })}
            >
              <option value="">auto</option>
              <option value="en">en</option>
              <option value="zh">zh</option>
            </Select>
          </Field>
        </div>
      </Card>

      <Card className="mb-4">
        <div className="mb-3 text-sm font-semibold text-zinc-200">Proxy</div>
        <div className="grid gap-x-4 sm:grid-cols-2">
          <Field label="Host">
            <Input value={s.proxy.host} onChange={(e) => setProxy({ host: e.target.value })} />
          </Field>
          <Field label="Port">
            <Input
              type="number"
              value={s.proxy.port}
              onChange={(e) => setProxy({ port: parseInt(e.target.value, 10) || 0 })}
            />
          </Field>
          <Field label="Global User-Agent disguise">
            <Select
              value={s.proxy.userAgent ?? ""}
              onChange={(e) => setProxy({ userAgent: e.target.value || undefined })}
            >
              <option value="">none</option>
              <option value="claude-code">claude-code</option>
              <option value="codex">codex</option>
              <option value="gemini">gemini</option>
            </Select>
          </Field>
        </div>

        <div className="mt-2 rounded-lg border border-white/10 p-3">
          <label className="flex items-center gap-2 text-sm text-zinc-300">
            <input
              type="checkbox"
              checked={s.proxy.circuitBreaker.enabled}
              onChange={(e) => setCb({ enabled: e.target.checked })}
            />
            Circuit breaker enabled
          </label>
          <div className="mt-3 grid gap-x-4 sm:grid-cols-2">
            <Field label="Failure threshold">
              <Input
                type="number"
                value={s.proxy.circuitBreaker.failureThreshold}
                onChange={(e) =>
                  setCb({ failureThreshold: parseInt(e.target.value, 10) || 0 })
                }
              />
            </Field>
            <Field label="Cooldown (seconds)">
              <Input
                type="number"
                value={s.proxy.circuitBreaker.cooldownSeconds}
                onChange={(e) =>
                  setCb({ cooldownSeconds: parseInt(e.target.value, 10) || 0 })
                }
              />
            </Field>
          </div>
        </div>
      </Card>

      <Card className="mb-4">
        <div className="mb-3 text-sm font-semibold text-zinc-200">Web UI</div>
        <div className="grid gap-x-4 sm:grid-cols-2">
          <Field label="Host">
            <Input value={s.web.host} onChange={(e) => setWeb({ host: e.target.value })} />
          </Field>
          <Field label="Port">
            <Input
              type="number"
              value={s.web.port}
              onChange={(e) => setWeb({ port: parseInt(e.target.value, 10) || 0 })}
            />
          </Field>
        </div>
        <div className="text-xs text-zinc-500">
          Non-loopback hosts require Basic auth (password in ~/.pi-switch/webui_password). Changes
          take effect on next <code>webui start</code>.
        </div>
      </Card>

      <div className="flex justify-end">
        <Button variant="primary" onClick={() => run(() => api.updateSettings(s), "Settings saved", refresh)}>
          Save settings
        </Button>
      </div>
    </div>
  );
}
