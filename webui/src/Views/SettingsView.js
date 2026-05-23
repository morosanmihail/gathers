import React, { useState, useEffect } from "react";
import { CollectionsProvider } from "../Components/CollectionContext";
import { SystemTypeProvider } from "../Components/SystemTypeContext";
import Header from "../Components/Layout/Header";

const SYSTEM_LABELS = {
  Sql: "Magic: The Gathering (SQLite)",
  Scryfall: "Magic: The Gathering (Scryfall)",
  RiftboundSql: "Riftbound (SQLite)",
  PokemonSql: "Pokémon (SQLite)",
};

const ALL_SYSTEMS = ["Sql", "Scryfall", "RiftboundSql", "PokemonSql"];

const PATH_FIELDS = [
  { key: "mtg_db_path",       label: "MTG Database path" },
  { key: "mtg_prices_path",   label: "MTG Prices path" },
  { key: "riftbound_db_path", label: "Riftbound Database path" },
  { key: "pokemon_db_path",   label: "Pokémon Database path" },
  { key: "storage_db_path",   label: "Storage Database path" },
];

export default function SettingsView() {
  const [config, setConfig] = useState(null);
  const [error, setError] = useState(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [demoMode, setDemoMode] = useState(false);

  useEffect(() => {
    fetch("/settings")
      .then((r) => {
        if (r.status === 403) {
          setDemoMode(true);
          return null;
        }
        if (!r.ok) throw new Error(`Failed to load settings (${r.status})`);
        return r.json();
      })
      .then((data) => {
        if (data) setConfig(data);
      })
      .catch((e) => setError(e.message));
  }, []);

  const toggleSystem = (system) => {
    setConfig((prev) => {
      const has = prev.system.includes(system);
      return {
        ...prev,
        system: has ? prev.system.filter((s) => s !== system) : [...prev.system, system],
      };
    });
    setSaved(false);
  };

  const setPath = (key, value) => {
    setConfig((prev) => ({ ...prev, [key]: value || null }));
    setSaved(false);
  };

  const setPort = (value) => {
    setConfig((prev) => ({ ...prev, port: parseInt(value) || prev.port }));
    setSaved(false);
  };

  const save = () => {
    setSaving(true);
    setSaved(false);
    setError(null);
    fetch("/settings", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(config),
    })
      .then((r) => {
        if (!r.ok) return r.json().then((b) => { throw new Error(b.error || `Save failed (${r.status})`); });
        return r.json();
      })
      .then((data) => {
        setConfig(data);
        setSaved(true);
      })
      .catch((e) => setError(e.message))
      .finally(() => setSaving(false));
  };

  return (
    <CollectionsProvider>
      <SystemTypeProvider>
        <Header />
        <main>
      <div className="container-fluid py-4" style={{ maxWidth: 720 }}>
        <h4 className="mb-4">Settings</h4>

        {demoMode && (
          <div className="alert alert-warning">
            Settings are disabled in demo mode.
          </div>
        )}

        {error && (
          <div className="alert alert-danger">{error}</div>
        )}

        {!demoMode && !config && !error && (
          <p className="text-muted">Loading…</p>
        )}

        {config && (
          <>
            <div className="card border-secondary mb-4">
              <div className="card-header">Systems</div>
              <div className="card-body">
                {ALL_SYSTEMS.map((system) => (
                  <div key={system} className="form-check mb-2">
                    <input
                      type="checkbox"
                      id={`sys-${system}`}
                      className="form-check-input"
                      checked={config.system.includes(system)}
                      onChange={() => toggleSystem(system)}
                    />
                    <label htmlFor={`sys-${system}`} className="form-check-label">
                      {SYSTEM_LABELS[system] ?? system}
                    </label>
                  </div>
                ))}
              </div>
            </div>

            <div className="card border-secondary mb-4">
              <div className="card-header">Server</div>
              <div className="card-body">
                <div className="mb-3">
                  <label className="form-label small text-muted">Port</label>
                  <input
                    type="number"
                    className="form-control form-control-sm"
                    value={config.port}
                    onChange={(e) => setPort(e.target.value)}
                    style={{ maxWidth: 120 }}
                  />
                </div>
              </div>
            </div>

            <div className="card border-secondary mb-4">
              <div className="card-header">File paths</div>
              <div className="card-body">
                {PATH_FIELDS.map(({ key, label }) => (
                  <div key={key} className="mb-3">
                    <label className="form-label small text-muted">{label}</label>
                    <input
                      type="text"
                      className="form-control form-control-sm font-monospace"
                      value={config[key] ?? ""}
                      onChange={(e) => setPath(key, e.target.value)}
                      placeholder="(not set)"
                    />
                  </div>
                ))}
              </div>
            </div>

            <div className="d-flex align-items-center gap-3">
              <button
                className="btn btn-primary"
                onClick={save}
                disabled={saving}
              >
                {saving ? "Saving…" : "Save"}
              </button>
              {saved && <span className="text-success small">Saved. Restart the server for changes to take effect.</span>}
            </div>
          </>
        )}
      </div>
        </main>
      </SystemTypeProvider>
    </CollectionsProvider>
  );
}
