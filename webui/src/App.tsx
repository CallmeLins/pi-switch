import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { AppState } from "./types";
import { Button, ToastProvider, cx } from "./components/ui";
import { HomePanel } from "./components/HomePanel";
import { ProfilesPanel } from "./components/ProfilesPanel";
import { ProxyPanel } from "./components/ProxyPanel";
import { StatsPanel } from "./components/StatsPanel";
import { BackupsPanel } from "./components/BackupsPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { DoctorPanel } from "./components/DoctorPanel";

type NavKey = "home" | "profiles" | "proxy" | "stats" | "backups" | "settings" | "doctor";

const NAV: { key: NavKey; label: string; icon: string }[] = [
  { key: "home", label: "Home", icon: "🏠" },
  { key: "profiles", label: "Profiles", icon: "👤" },
  { key: "proxy", label: "Proxy", icon: "🔄" },
  { key: "stats", label: "Stats", icon: "📊" },
  { key: "backups", label: "Backups", icon: "💾" },
  { key: "settings", label: "Settings", icon: "⚙️" },
  { key: "doctor", label: "Doctor", icon: "🩺" },
];

export interface PanelProps {
  state: AppState;
  refresh: () => Promise<void>;
}

export default function App() {
  return (
    <ToastProvider>
      <Shell />
    </ToastProvider>
  );
}

function Shell() {
  const [nav, setNav] = useState<NavKey>("home");
  const [state, setState] = useState<AppState | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setState(await api.getState());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const initConfig = useCallback(async () => {
    await api.init();
    await refresh();
  }, [refresh]);

  return (
    <div className="flex h-full">
      {/* Sidebar */}
      <aside className="flex w-56 shrink-0 flex-col border-r border-white/10 bg-zinc-950/60">
        <div className="px-4 py-4">
          <div className="text-lg font-bold tracking-tight text-zinc-100">pi-switch</div>
          <div className="text-[11px] text-zinc-500">provider control · web</div>
        </div>
        <nav className="flex-1 px-2">
          {NAV.map((item) => (
            <button
              key={item.key}
              onClick={() => setNav(item.key)}
              className={cx(
                "mb-0.5 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm",
                nav === item.key
                  ? "bg-indigo-600/20 text-indigo-200"
                  : "text-zinc-400 hover:bg-white/5 hover:text-zinc-200",
              )}
            >
              <span className="text-base">{item.icon}</span>
              {item.label}
            </button>
          ))}
        </nav>
        <div className="px-4 py-3 text-[11px] text-zinc-600">
          CLI · TUI · WebUI — same core
        </div>
      </aside>

      {/* Main */}
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-5xl px-6 py-6">
          {error && (
            <div className="mb-4 rounded-lg border border-red-500/30 bg-red-950/40 px-4 py-3 text-sm text-red-200">
              <div className="font-medium">Could not load config</div>
              <div className="mt-1 text-red-300/80">{error}</div>
              <Button variant="primary" className="mt-3" onClick={() => void initConfig()}>
                Initialize config
              </Button>
            </div>
          )}

          {!state && !error && <div className="text-zinc-500">Loading…</div>}

          {state && (
            <>
              {nav === "home" && <HomePanel state={state} refresh={refresh} onNavigate={setNav} />}
              {nav === "profiles" && <ProfilesPanel state={state} refresh={refresh} />}
              {nav === "proxy" && <ProxyPanel state={state} refresh={refresh} />}
              {nav === "stats" && <StatsPanel state={state} refresh={refresh} />}
              {nav === "backups" && <BackupsPanel state={state} refresh={refresh} />}
              {nav === "settings" && <SettingsPanel state={state} refresh={refresh} />}
              {nav === "doctor" && <DoctorPanel state={state} refresh={refresh} />}
            </>
          )}
        </div>
      </main>
    </div>
  );
}
