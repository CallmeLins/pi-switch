import { useState } from "react";
import type { AppState, Settings } from "../types";
import { api } from "../api";
import { Button, Card, Field, Input, SectionTitle, Select, useAction } from "./ui";
import { useI18n } from "../i18n";

export function SettingsPanel({
  state,
  refresh,
}: {
  state: AppState;
  refresh: () => Promise<void>;
}) {
  const run = useAction();
  const { t, lang, setLang } = useI18n();
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
      <SectionTitle hint={t("written to ~/.pi-switch/config.json")}>{t("Settings")}</SectionTitle>

      <Card className="mb-4">
        <div className="mb-3 text-sm font-semibold text-zinc-200">{t("General")}</div>
        <div className="grid gap-x-4 sm:grid-cols-2">
          <Field label={t("Provider prefix (pi gateway id)")}>
            <Input
              value={s.providerPrefix}
              onChange={(e) => set({ providerPrefix: e.target.value })}
            />
          </Field>
          <Field label={t("Write mode")}>
            <Select value={s.writeMode} onChange={(e) => set({ writeMode: e.target.value })}>
              <option value="merge">merge</option>
              <option value="exclusive">exclusive</option>
            </Select>
          </Field>
          <Field label={t("Language")}>
            <Select
              value={s.language ?? ""}
              onChange={(e) => {
                const v = e.target.value || null;
                set({ language: v });
                // Apply immediately (WebUI now has real i18n).
                if (v === "zh") setLang("zh");
                else if (v === "en") setLang("en");
                else setLang(navigator.language.startsWith("zh") ? "zh" : "en");
              }}
            >
              <option value="">{t("auto")}</option>
              <option value="en">en</option>
              <option value="zh">zh</option>
            </Select>
          </Field>
          <Field label={t("Current UI language")}>
            <Input value={lang === "zh" ? "中文" : "English"} readOnly />
          </Field>
        </div>
      </Card>

      <Card className="mb-4">
        <div className="mb-3 text-sm font-semibold text-zinc-200">{t("Proxy")}</div>
        <div className="grid gap-x-4 sm:grid-cols-2">
          <Field label={t("Proxy host")}>
            <Input value={s.proxy.host} onChange={(e) => setProxy({ host: e.target.value })} />
          </Field>
          <Field label={t("Proxy port")}>
            <Input
              type="number"
              value={s.proxy.port}
              onChange={(e) => setProxy({ port: parseInt(e.target.value, 10) || 0 })}
            />
          </Field>
          <Field label={t("Global User-Agent disguise")}>
            <Select
              value={s.proxy.userAgent ?? ""}
              onChange={(e) => setProxy({ userAgent: e.target.value || undefined })}
            >
              <option value="">{t("none")}</option>
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
            {t("Circuit breaker enabled")}
          </label>
          <div className="mt-3 grid gap-x-4 sm:grid-cols-2">
            <Field label={t("Failure threshold")}>
              <Input
                type="number"
                value={s.proxy.circuitBreaker.failureThreshold}
                onChange={(e) =>
                  setCb({ failureThreshold: parseInt(e.target.value, 10) || 0 })
                }
              />
            </Field>
            <Field label={t("Cooldown (seconds)")}>
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
        <div className="mb-3 text-sm font-semibold text-zinc-200">{t("Web UI")}</div>
        <div className="grid gap-x-4 sm:grid-cols-2">
          <Field label={t("Proxy host")}>
            <Input value={s.web.host} onChange={(e) => setWeb({ host: e.target.value })} />
          </Field>
          <Field label={t("Proxy port")}>
            <Input
              type="number"
              value={s.web.port}
              onChange={(e) => setWeb({ port: parseInt(e.target.value, 10) || 0 })}
            />
          </Field>
        </div>
        <div className="text-xs text-zinc-500">
          {t("Non-loopback hosts require Basic auth (password in ~/.pi-switch/webui_password). Changes take effect on next webui start.")}
        </div>
      </Card>

      <div className="flex justify-end">
        <Button
          variant="primary"
          onClick={() => run(() => api.updateSettings(s), t("Settings saved"), refresh)}
        >
          {t("Save settings")}
        </Button>
      </div>
    </div>
  );
}
