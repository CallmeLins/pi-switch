import { useEffect, useState } from "react";
import type { AppState } from "../types";
import { api } from "../api";
import { Button, Card, Field, Input, SectionTitle, useAction } from "./ui";

export function BackupsPanel({
  refresh,
}: {
  state: AppState;
  refresh: () => Promise<void>;
}) {
  const run = useAction();
  const [backups, setBackups] = useState<string[]>([]);

  const load = async () => {
    try {
      setBackups(await api.backups());
    } catch {
      setBackups([]);
    }
  };
  useEffect(() => {
    void load();
  }, []);

  const [passphrase, setPassphrase] = useState("");
  const [importPath, setImportPath] = useState("");
  const [importPass, setImportPass] = useState("");

  return (
    <div>
      <SectionTitle hint="config backups & encrypted sync">Backups</SectionTitle>

      <Card className="mb-4">
        <div className="mb-2 flex items-center justify-between">
          <div className="text-sm font-semibold text-zinc-200">Config backups</div>
          <Button onClick={() => void load()}>Refresh</Button>
        </div>
        <div className="max-h-72 space-y-1 overflow-y-auto">
          {backups.length === 0 && <div className="text-sm text-zinc-500">No backups yet.</div>}
          {backups
            .slice()
            .reverse()
            .map((path) => (
              <div
                key={path}
                className="flex items-center justify-between gap-2 rounded-md border border-white/10 px-2 py-1.5"
              >
                <span className="truncate font-mono text-xs text-zinc-400">{path}</span>
                <Button
                  onClick={() => {
                    if (confirm("Restore this backup? Current config is backed up first."))
                      run(() => api.restoreConfig(path), "Restored", refresh);
                  }}
                >
                  Restore
                </Button>
              </div>
            ))}
        </div>
      </Card>

      <div className="grid gap-4 sm:grid-cols-2">
        <Card>
          <div className="mb-2 text-sm font-semibold text-zinc-200">Export (encrypted)</div>
          <Field label="Passphrase">
            <Input
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              placeholder="passphrase"
            />
          </Field>
          <Button
            variant="primary"
            onClick={() =>
              run(
                async () => {
                  const r = await api.exportConfig(passphrase);
                  alert(`Exported to:\n${r.path}`);
                },
                "Exported",
              )
            }
          >
            Export config
          </Button>
        </Card>

        <Card>
          <div className="mb-2 text-sm font-semibold text-zinc-200">Import (encrypted)</div>
          <Field label="File path">
            <Input
              value={importPath}
              onChange={(e) => setImportPath(e.target.value)}
              placeholder="/path/to/export.enc"
            />
          </Field>
          <Field label="Passphrase">
            <Input
              type="password"
              value={importPass}
              onChange={(e) => setImportPass(e.target.value)}
            />
          </Field>
          <Button
            variant="primary"
            onClick={() =>
              run(() => api.importConfig(importPath, importPass), "Imported", refresh)
            }
          >
            Import config
          </Button>
        </Card>
      </div>
    </div>
  );
}
