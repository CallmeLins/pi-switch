import { useEffect, useState } from "react";
import { api } from "../api";
import type { PackageEntry } from "../types";
import { Button, Card, Input, SectionTitle, Switch, useAction } from "./ui";

interface PackagesPanelProps {
  refresh: () => void;
}

export function PackagesPanel({ refresh }: PackagesPanelProps) {
  const [packages, setPackages] = useState<PackageEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [adding, setAdding] = useState(false);
  const [formData, setFormData] = useState({ id: "", name: "", version: "" });
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
    if (!formData.id || !formData.name || !formData.version) {
      alert("All fields are required");
      return;
    }

    await run(
      () => api.addPackage(formData.id, formData.name, formData.version),
      `Package '${formData.name}' added`,
      () => {
        setAdding(false);
        setFormData({ id: "", name: "", version: "" });
        loadPackages();
        refresh();
      }
    );
  };

  const handleToggle = async (pkg: PackageEntry) => {
    await run(
      () => api.togglePackage(pkg.id),
      `Package '${pkg.name}' ${pkg.enabled ? "disabled" : "enabled"}`,
      () => {
        loadPackages();
        refresh();
      }
    );
  };

  const handleDelete = async (pkg: PackageEntry) => {
    if (!confirm(`Delete package '${pkg.name}'?`)) return;

    await run(
      () => api.deletePackage(pkg.id),
      `Package '${pkg.name}' deleted`,
      () => {
        loadPackages();
        refresh();
      }
    );
  };

  const handleImport = async () => {
    await run(
      () => api.importPackages(),
      "Packages imported from Pi Agent",
      () => {
        loadPackages();
        refresh();
      }
    );
  };

  if (loading) {
    return (
      <div>
        <SectionTitle>📦 Packages</SectionTitle>
        <p style={{ color: "#999" }}>Loading...</p>
      </div>
    );
  }

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "1.5rem" }}>
        <SectionTitle>📦 Packages</SectionTitle>
        <div style={{ display: "flex", gap: "0.5rem" }}>
          <Button onClick={handleImport}>📥 Import from Pi Agent</Button>
          <Button onClick={() => setAdding(!adding)}>
            {adding ? "Cancel" : "+ Add Package"}
          </Button>
        </div>
      </div>

      {adding && (
        <Card style={{ marginBottom: "1.5rem", padding: "1.5rem" }}>
          <h3 style={{ margin: "0 0 1rem 0", fontSize: "1.1rem" }}>Add New Package</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontSize: "0.9rem", color: "#666" }}>
                Package ID
              </label>
              <Input
                value={formData.id}
                onChange={(e) => setFormData({ ...formData, id: e.target.value })}
                placeholder="e.g., my-plugin"
              />
            </div>
            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontSize: "0.9rem", color: "#666" }}>
                Name
              </label>
              <Input
                value={formData.name}
                onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                placeholder="e.g., My Plugin"
              />
            </div>
            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontSize: "0.9rem", color: "#666" }}>
                Version
              </label>
              <Input
                value={formData.version}
                onChange={(e) => setFormData({ ...formData, version: e.target.value })}
                placeholder="e.g., 1.0.0"
              />
            </div>
            <Button onClick={handleAdd} style={{ marginTop: "0.5rem" }}>
              Add Package
            </Button>
          </div>
        </Card>
      )}

      {packages.length === 0 ? (
        <Card style={{ padding: "2rem", textAlign: "center", color: "#999" }}>
          <p>No packages installed.</p>
          <p style={{ fontSize: "0.9rem", marginTop: "0.5rem" }}>
            Click "Add Package" above or use CLI: <code>pi-switch package add &lt;id&gt; &lt;name&gt; &lt;version&gt;</code>
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
                    ID: <code style={{ background: "#f5f5f5", padding: "0.2rem 0.4rem", borderRadius: "3px" }}>{pkg.id}</code>
                    {pkg.installedAt && ` • Installed: ${new Date(pkg.installedAt).toLocaleString()}`}
                  </div>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                    <span style={{ fontSize: "0.9rem", color: "#666" }}>
                      {pkg.enabled ? "Enabled" : "Disabled"}
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
                    Delete
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
