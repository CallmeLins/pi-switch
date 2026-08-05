import { useEffect, useState } from "react";
import type { AppState, DaemonResult } from "../types";
import { api } from "../api";
import { Badge, Button, Card, SectionTitle } from "./ui";
import { useI18n } from "../i18n";

export function HomePanel({
  state,
  onNavigate,
}: {
  state: AppState;
  refresh: () => Promise<void>;
  onNavigate: (k: any) => void;
}) {
  const { t } = useI18n();
  const [proxy, setProxy] = useState<DaemonResult | null>(null);
  const profiles = Object.entries(state.profiles);
  const exposedCount = profiles.filter(
    ([, p]) => (p.exposedModels?.length ?? 0) > 0,
  ).length;

  useEffect(() => {
    api.proxyStatus().then(setProxy).catch(() => setProxy(null));
  }, []);

  return (
    <div>
      <SectionTitle hint={t("CLI / TUI / WebUI share one Rust core")}>
        {t("Overview")}
      </SectionTitle>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Stat label={t("Profiles")} value={String(profiles.length)} />
        <Stat label={t("Exposed")} value={String(exposedCount)} />
        <Stat
          label={t("Current")}
          value={state.current || "—"}
        />
        <Stat
          label={t("Proxy")}
          value={proxy?.running ? t("running") : t("stopped")}
          tone={proxy?.running ? "green" : "zinc"}
        />
      </div>

      <div className="mt-5 grid gap-3 sm:grid-cols-2">
        <Card>
          <div className="mb-2 text-sm font-semibold text-zinc-200">{t("Gateway workflow")}</div>
          <ol className="ml-4 list-decimal space-y-1 text-sm text-zinc-400">
            <li>{t("Add profiles & set API keys")}</li>
            <li>{t("Expose models to pi (per profile)")}</li>
            <li>{t("Optionally set a failover chain")}</li>
            <li>
              {t("Start the proxy — pi routes by profile/model")}{" "}
              <code>profile/model</code>
            </li>
          </ol>
          <div className="mt-3 flex gap-2">
            <Button variant="primary" onClick={() => onNavigate("profiles")}>
              {t("Manage profiles")}
            </Button>
            <Button onClick={() => onNavigate("proxy")}>{t("Proxy control")}</Button>
          </div>
        </Card>

        <Card>
          <div className="mb-2 text-sm font-semibold text-zinc-200">{t("Current selection")}</div>
          {state.current ? (
            <div className="text-sm text-zinc-400">
              {t("Active profile:")} <Badge tone="indigo">{state.current}</Badge>
              <div className="mt-2 text-xs text-zinc-500">
                {t("Provider id:")} {state.settings.providerPrefix}
              </div>
            </div>
          ) : (
            <div className="text-sm text-zinc-500">{t("No profile selected yet.")}</div>
          )}
          {proxy && (
            <div className="mt-3 text-xs text-zinc-500">{proxy.message}</div>
          )}
        </Card>
      </div>
    </div>
  );
}

function Stat({
  label,
  value,
  tone = "zinc",
}: {
  label: string;
  value: string;
  tone?: "zinc" | "green";
}) {
  return (
    <Card className="py-3">
      <div className="text-[11px] uppercase tracking-wide text-zinc-500">{label}</div>
      <div
        className={
          "mt-1 truncate text-xl font-semibold " +
          (tone === "green" ? "text-emerald-300" : "text-zinc-100")
        }
      >
        {value}
      </div>
    </Card>
  );
}
