import { useEffect, useState } from "react";
import type { AppState, DoctorCheck, ValidationIssue } from "../types";
import { api } from "../api";
import { Badge, Button, Card, SectionTitle } from "./ui";
import { useI18n } from "../i18n";

export function DoctorPanel(_: { state: AppState; refresh: () => Promise<void> }) {
  const { t } = useI18n();
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
      <SectionTitle hint={t("config & connectivity checks")}>{t("Doctor")}</SectionTitle>

      <div className="mb-3">
        <Button onClick={() => void load()}>{t("Re-run")}</Button>
      </div>

      <Card className="mb-4">
        <div className="mb-2 text-sm font-semibold text-zinc-200">{t("Health checks")}</div>
        <div className="space-y-1">
          {checks.map((c, i) => (
            <div key={i} className="flex items-center gap-2 text-sm">
              <span className={c.ok ? "text-emerald-400" : "text-red-400"}>
                {c.ok ? "✓" : "✗"}
              </span>
              <span className="text-zinc-300">{c.msg}</span>
            </div>
          ))}
          {checks.length === 0 && <div className="text-sm text-zinc-500">{t("No checks.")}</div>}
        </div>
      </Card>

      <Card>
        <div className="mb-2 text-sm font-semibold text-zinc-200">{t("Validation")}</div>
        {issues.length === 0 ? (
          <div className="text-sm text-emerald-300">{t("No issues found.")}</div>
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
