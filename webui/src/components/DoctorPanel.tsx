import { useEffect, useState } from "react";
import type { AppState, DoctorCheck, ValidationIssue } from "../types";
import { api } from "../api";
import { Badge, Button, Card, SectionTitle } from "./ui";

export function DoctorPanel(_: { state: AppState; refresh: () => Promise<void> }) {
  const [checks, setChecks] = useState<DoctorCheck[]>([]);
  const [issues, setIssues] = useState<ValidationIssue[]>([]);

  const load = async () => {
    const [c, v] = await Promise.all([
      api.doctor().catch(() => []),
      api.validate().catch(() => []),
    ]);
    setChecks(c);
    setIssues(v);
  };
  useEffect(() => {
    void load();
  }, []);

  return (
    <div>
      <SectionTitle hint="config & connectivity checks">Doctor</SectionTitle>

      <div className="mb-3">
        <Button onClick={() => void load()}>Re-run</Button>
      </div>

      <Card className="mb-4">
        <div className="mb-2 text-sm font-semibold text-zinc-200">Health checks</div>
        <div className="space-y-1">
          {checks.map((c, i) => (
            <div key={i} className="flex items-center gap-2 text-sm">
              <span className={c.ok ? "text-emerald-400" : "text-red-400"}>
                {c.ok ? "✓" : "✗"}
              </span>
              <span className="text-zinc-300">{c.msg}</span>
            </div>
          ))}
          {checks.length === 0 && <div className="text-sm text-zinc-500">No checks.</div>}
        </div>
      </Card>

      <Card>
        <div className="mb-2 text-sm font-semibold text-zinc-200">Validation</div>
        {issues.length === 0 ? (
          <div className="text-sm text-emerald-300">No issues found.</div>
        ) : (
          <div className="space-y-1">
            {issues.map((iss, i) => (
              <div key={i} className="flex items-start gap-2 text-sm">
                <Badge tone={iss.level === "error" ? "red" : "amber"}>{iss.level}</Badge>
                <span className="text-zinc-400">
                  <span className="font-mono text-zinc-500">{iss.path}</span> — {iss.message}
                </span>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
