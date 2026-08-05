import { useEffect, useState } from "react";
import { api } from "../api";
import type { PackageEntry } from "../types";
import { Button, Card, Input, SectionTitle, Switch, useAction } from "./ui";
import { useI18n } from "../i18n";

interface PackagesPanelProps {
  refresh: () => void;
}

function CapBadge({ label }: { label: string }) {
  return (
    <span
      style={{
        fontSize: "0.75rem",
        color: "#3B82F6",
        background: "#DBEAFE",
        padding: "0.15rem 0.5rem",
        borderRadius: "999px",
      }}
    >
      {label}
    </span>
  );
}

export function PackagesPanel({ refresh }: PackagesPanelProps) {
  const { t } = useI18n();
  const [packages, setPackages] = useState<PackageEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [adding, setAdding] = useState(false);
  const [spec, setSpec] = useState("");
  const run = useAction();

  const loadPackages = async () => {
    try {
      setLoading(true);
      const data = await api.getPackages();
      setPackages(data.packages);
    } catch (err) {
      console.error("Failed to load packages:", err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadPackages();
  }, []);

  const handleAdd = async () => {
    if (!spec.trim()) {
      alert(t("Package spec is required"));
      return;
    }

    await run(
      () => api.addPackage(spec.trim()),
      t("Package '{{spec}}' installed").replace("{{spec}}", spec.trim()),
      () => {
        setAdding(false);
        setSpec("");
        loadPackages();
        refresh();
      }
    );
  };

  const handleToggle = async (pkg: PackageEntry) => {
    await run(
      () => api.togglePackage(pkg.id),
      pkg.enabled
        ? t("Package '{{name}}' disabled").replace("{{name}}", pkg.name)
        : t("Package '{{name}}' enabled").replace("{{name}}", pkg.name),
      () => {
        loadPackages();
        refresh();
      }
    );
  };

  const handleDelete = async (pkg: PackageEntry) => {
    if (!confirm(t("Uninstall package '{{name}}'?").replace("{{name}}", pkg.name))) return;

    await run(
      () => api.deletePackage(pkg.id),
      t("Package '{{name}}' deleted").replace("{{name}}", pkg.name),
      () => {
        loadPackages();
        refresh();
      }
    );
  };

  const handleImport = async () => {
    await run(
      () => api.importPackages(),
      t("Packages imported from Pi Agent"),
      () => {
        loadPackages();
        refresh();
      }
    );
  };

  if (loading) {
    return (
      <div>
        <SectionTitle>📦 {t("Packages")}</SectionTitle>
        <p style={{ color: "#999" }}>{t("Loading…")}</p>
      </div>
    );
  }

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "1.5rem" }}>
        <SectionTitle>📦 {t("Packages")}</SectionTitle>
        <div style={{ display: "flex", gap: "0.5rem" }}>
          <Button onClick={handleImport}>📥 {t("Import from Pi Agent")}</Button>
          <Button onClick={() => setAdding(!adding)}>
            {adding ? t("Cancel") : t("+ Add Package")}
          </Button>
        </div>
      </div>

      {adding && (
        <Card style={{ marginBottom: "1.5rem", padding: "1.5rem" }}>
          <h3 style={{ margin: "0 0 1rem 0", fontSize: "1.1rem" }}>{t("Install Package")}</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontSize: "0.9rem", color: "#666" }}>
                {t("Spec")}
              </label>
              <Input
                value={spec}
                onChange={(e) => setSpec(e.target.value)}
                placeholder={t("e.g., npm:foo@1.0.0, git:github.com/user/repo, or local path")}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleAdd();
                }}
              />
            </div>
            <Button onClick={handleAdd} style={{ marginTop: "0.5rem" }}>
              {t("Install")}
            </Button>
          </div>
        </Card>
      )}

      {packages.length === 0 ? (
        <Card style={{ padding: "2rem", textAlign: "center", color: "#999" }}>
          <p>{t("No packages installed.")}</p>
          <p style={{ fontSize: "0.9rem", marginTop: "0.5rem" }}>
            {t('Click "Add Package" above or use CLI: pi-switch package add <id> <name> <version>')}
          </p>
        </Card>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          {packages.map((pkg) => (
            <Card key={pkg.id} style={{ padding: "1.5rem" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div style={{ flex: 1 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "0.5rem" }}>
                    <h3 style={{ margin: 0, fontSize: "1.1rem" }}>{pkg.name}</h3>
                    <span
                      style={{
                        fontSize: "0.85rem",
                        color: "#C4612F",
                        background: "#F2E3D6",
                        padding: "0.25rem 0.5rem",
                        borderRadius: "999px",
                      }}
                    >
                      v{pkg.version}
                    </span>
                  </div>
                  <div style={{ fontSize: "0.85rem", color: "#999" }}>
                    {t("ID")}: <code style={{ background: "#f5f5f5", padding: "0.2rem 0.4rem", borderRadius: "3px" }}>{pkg.id}</code>
                    {pkg.installedAt && ` • ${t("Installed:")} ${new Date(pkg.installedAt).toLocaleString()}`}
                  </div>
                  {(pkg.hasExtensions || pkg.hasSkills || pkg.hasPrompts || pkg.hasThemes) && (
                    <div style={{ display: "flex", gap: "0.4rem", marginTop: "0.5rem" }}>
                      {pkg.hasExtensions && <CapBadge label={t("extensions")} />}
                      {pkg.hasSkills && <CapBadge label={t("skills")} />}
                      {pkg.hasPrompts && <CapBadge label={t("prompts")} />}
                      {pkg.hasThemes && <CapBadge label={t("themes")} />}
                    </div>
                  )}
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                    <span style={{ fontSize: "0.9rem", color: "#666" }}>
                      {pkg.enabled ? t("Enabled") : t("Disabled")}
                    </span>
                    <Switch checked={pkg.enabled} onChange={() => handleToggle(pkg)} />
                  </div>
                  <Button
                    onClick={() => handleDelete(pkg)}
                    style={{
                      background: "transparent",
                      color: "#ff5555",
                      border: "1px solid #ff5555",
                    }}
                  >
                    {t("Uninstall")}
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
