import { useEffect, useState } from "react";
import type { AppState, DaemonResult } from "../types";
import { api } from "../api";
import { Badge, Button, Card, SectionTitle } from "./ui";

export function HomePanel({
  state,
  onNavigate,
}: {
  state: AppState;
  refresh: () => Promise<void>;
  onNavigate: (k: any) => void;
}) {
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
      <SectionTitle hint="CLI / TUI / WebUI share one Rust core">Overview</SectionTitle>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Stat label="Profiles" value={String(profiles.length)} />
        <Stat label="Exposed" value={String(exposedCount)} />
        <Stat
          label="Current"
          value={state.current || "—"}
        />
        <Stat
          label="Proxy"
          value={proxy?.running ? "running" : "stopped"}
          tone={proxy?.running ? "green" : "zinc"}
        />
      </div>

      <div className="mt-5 grid gap-3 sm:grid-cols-2">
        <Card>
          <div className="mb-2 text-sm font-semibold text-zinc-200">Gateway workflow</div>
          <ol className="ml-4 list-decimal space-y-1 text-sm text-zinc-400">
            <li>Add profiles &amp; set API keys</li>
            <li>Expose models to pi (per profile)</li>
            <li>Optionally set a failover chain</li>
            <li>Start the proxy — pi routes by <code>profile/model</code></li>
          </ol>
          <div className="mt-3 flex gap-2">
            <Button variant="primary" onClick={() => onNavigate("profiles")}>
              Manage profiles
            </Button>
            <Button onClick={() => onNavigate("proxy")}>Proxy control</Button>
          </div>
        </Card>

        <Card>
          <div className="mb-2 text-sm font-semibold text-zinc-200">Current selection</div>
          {state.current ? (
            <div className="text-sm text-zinc-400">
              Active profile: <Badge tone="indigo">{state.current}</Badge>
              <div className="mt-2 text-xs text-zinc-500">
                Provider id: {state.settings.providerPrefix}
              </div>
            </div>
          ) : (
            <div className="text-sm text-zinc-500">No profile selected yet.</div>
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
