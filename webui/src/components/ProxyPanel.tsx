import { useEffect, useState } from "react";
import type { AppState, DaemonResult } from "../types";
import { api } from "../api";
import { Badge, Button, Card, Field, Input, SectionTitle } from "./ui";
import { useAction } from "./ui";

export function ProxyPanel({
  state,
  refresh,
}: {
  state: AppState;
  refresh: () => Promise<void>;
}) {
  const run = useAction();
  const [status, setStatus] = useState<DaemonResult | null>(null);
  const [host, setHost] = useState(state.settings.proxy.host);
  const [port, setPort] = useState(String(state.settings.proxy.port));

  const loadStatus = async () => {
    try {
      setStatus(await api.proxyStatus());
    } catch {
      setStatus(null);
    }
  };
  useEffect(() => {
    void loadStatus();
  }, []);

  return (
    <div>
      <SectionTitle hint="routes by profile/model in the request body">Proxy</SectionTitle>

      <Card className="mb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Badge tone={status?.running ? "green" : "zinc"}>
              {status?.running ? "running" : "stopped"}
            </Badge>
            {status?.running && (
              <span className="text-sm text-zinc-400">
                PID {status.pid} · http://{status.host}:{status.port}
              </span>
            )}
          </div>
          <Button onClick={() => void loadStatus()}>Refresh</Button>
        </div>
        {status?.message && <div className="mt-2 text-xs text-zinc-500">{status.message}</div>}

        <div className="mt-4 grid gap-x-4 sm:grid-cols-2">
          <Field label="Host">
            <Input value={host} onChange={(e) => setHost(e.target.value)} />
          </Field>
          <Field label="Port">
            <Input value={port} onChange={(e) => setPort(e.target.value)} />
          </Field>
        </div>
        <div className="flex gap-2">
          <Button
            variant="primary"
            disabled={status?.running}
            onClick={() =>
              run(
                () => api.proxyStart(host.trim(), parseInt(port, 10) || undefined),
                "Proxy started",
                loadStatus,
              )
            }
          >
            Start
          </Button>
          <Button
            variant="danger"
            disabled={!status?.running}
            onClick={() => run(() => api.proxyStop(), "Proxy stopped", loadStatus)}
          >
            Stop
          </Button>
        </div>
      </Card>

      <FailoverEditor state={state} refresh={refresh} />
    </div>
  );
}

function FailoverEditor({
  state,
  refresh,
}: {
  state: AppState;
  refresh: () => Promise<void>;
}) {
  const run = useAction();
  const [chain, setChain] = useState<string[]>(state.settings.proxy.failover ?? []);

  const nonProxy = Object.entries(state.profiles)
    .filter(([, p]) => !p.proxy)
    .map(([n]) => n);
  const available = nonProxy.filter((n) => !chain.includes(n));

  const move = (i: number, d: number) => {
    const j = i + d;
    if (j < 0 || j >= chain.length) return;
    const next = [...chain];
    [next[i], next[j]] = [next[j], next[i]];
    setChain(next);
  };

  return (
    <Card>
      <div className="mb-1 text-sm font-semibold text-zinc-200">Failover chain</div>
      <div className="mb-3 text-xs text-zinc-500">
        Same-model fallback order when the primary provider fails. Proxy profiles are excluded.
      </div>

      <div className="space-y-1">
        {chain.length === 0 && (
          <div className="text-sm text-zinc-500">No failover configured.</div>
        )}
        {chain.map((name, i) => (
          <div
            key={name}
            className="flex items-center justify-between rounded-md border border-white/10 px-2 py-1.5 text-sm"
          >
            <span className="text-zinc-200">
              <span className="mr-2 text-zinc-500">{i + 1}.</span>
              {name}
            </span>
            <div className="flex gap-1">
              <button className="px-1 text-zinc-400 hover:text-zinc-100" onClick={() => move(i, -1)}>
                ↑
              </button>
              <button className="px-1 text-zinc-400 hover:text-zinc-100" onClick={() => move(i, 1)}>
                ↓
              </button>
              <button
                className="px-1 text-zinc-400 hover:text-red-300"
                onClick={() => setChain(chain.filter((x) => x !== name))}
              >
                ✕
              </button>
            </div>
          </div>
        ))}
      </div>

      <div className="mt-3 flex items-center gap-2">
        {available.length > 0 && (
          <select
            className="rounded-md border border-white/10 bg-zinc-950/60 px-2 py-1.5 text-sm text-zinc-100"
            value=""
            onChange={(e) => {
              if (e.target.value) setChain([...chain, e.target.value]);
            }}
          >
            <option value="">+ add profile…</option>
            {available.map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        )}
        <Button
          variant="primary"
          onClick={() => run(() => api.setFailover(chain), "Failover saved", refresh)}
        >
          Save chain
        </Button>
      </div>
    </Card>
  );
}
