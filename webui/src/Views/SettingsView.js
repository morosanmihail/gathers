import React, { useState, useEffect, useCallback } from "react";
import { CollectionsProvider } from "../Components/CollectionContext";
import { SystemTypeProvider, useRefreshSystemInfo } from "../Components/SystemTypeContext";
import Header from "../Components/Layout/Header";
import { useOperations } from "../OperationsContext";

const SYSTEM_LABELS = {
  Sql: "Magic: The Gathering (SQLite)",
  Scryfall: "Magic: The Gathering (Scryfall)",
  RiftboundSql: "Riftbound (SQLite)",
  PokemonSql: "Pokémon (SQLite)",
};

const ALL_SYSTEMS = ["Sql", "Scryfall", "RiftboundSql", "PokemonSql"];

const SYSTEM_ACTIONS = {
  Sql: [
    { label: "Update DB",     endpoint: "/api/mtg/update" },
    { label: "Update Prices", endpoint: "/api/mtg/prices/update" },
  ],
  RiftboundSql: [
    { label: "Update DB", endpoint: "/api/riftbound/update" },
  ],
  PokemonSql: [
    { label: "Update DB",     endpoint: "/api/pokemon/update" },
    { label: "Update Prices", endpoint: "/api/pokemon/prices/update" },
  ],
};

const PATH_FIELDS = [
  { key: "mtg_db_path",          label: "MTG Database path" },
  { key: "mtg_prices_path",      label: "MTG Prices path" },
  { key: "riftbound_db_path",    label: "Riftbound Database path" },
  { key: "pokemon_db_path",      label: "Pokémon Database path" },
  { key: "pokemon_prices_path",  label: "Pokémon Prices path" },
  { key: "storage_db_path",      label: "Storage Database path" },
];

function UpdateButton({ label, endpoint }) {
  const [status, setStatus] = useState(null);
  const [running, setRunning] = useState(false);
  const ops = useOperations();

  const run = useCallback(() => {
    setRunning(true);
    setStatus(null);
    ops.fetch(label, null, endpoint)
      .then((body) => setStatus({ ok: true, text: typeof body === "string" ? body : "Done" }))
      .catch((e) => setStatus({ ok: false, text: e.message }))
      .finally(() => setRunning(false));
  }, [ops, label, endpoint]);

  return (
    <span className="d-inline-flex align-items-center gap-2">
      <button
        className="btn btn-sm btn-outline-secondary"
        onClick={run}
        disabled={running}
      >
        {running ? "Running…" : label}
      </button>
      {status && (
        <span className={`small ${status.ok ? "text-success" : "text-danger"}`}>
          {status.text}
        </span>
      )}
    </span>
  );
}

function SettingsContent() {
  const refreshSystemInfo = useRefreshSystemInfo();
  const [config, setConfig] = useState(null);
  const [error, setError] = useState(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [demoMode, setDemoMode] = useState(false);

  useEffect(() => {
    fetch("/api/settings")
      .then((r) => {
        if (r.status === 403) { setDemoMode(true); return null; }
        if (!r.ok) throw new Error(`Failed to load settings (${r.status})`);
        return r.json();
      })
      .then((data) => { if (data) setConfig(data); })
      .catch((e) => setError(e.message));
  }, []);

  const toggleSystem = (system) => {
    setConfig((prev) => {
      const has = prev.system.includes(system);
      return { ...prev, system: has ? prev.system.filter((s) => s !== system) : [...prev.system, system] };
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

  const togglePricing = () => {
    setConfig((prev) => ({ ...prev, pricing_enabled: !prev.pricing_enabled }));
    setSaved(false);
  };

  const toggleCollections = () => {
    setConfig((prev) => ({ ...prev, collections_enabled: !prev.collections_enabled }));
    setSaved(false);
  };

  const save = () => {
    setSaving(true);
    setSaved(false);
    setError(null);
    fetch("/api/settings", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(config),
    })
      .then((r) => {
        if (!r.ok) return r.json().then((b) => { throw new Error(b.error || `Save failed (${r.status})`); });
        return r.json();
      })
      .then((data) => { setConfig(data); setSaved(true); refreshSystemInfo(); })
      .catch((e) => setError(e.message))
      .finally(() => setSaving(false));
  };

  return (
    <>
      <Header />
      <main>
          <div className="container-fluid py-4" style={{ maxWidth: 720 }}>
            <h4 className="mb-4">Settings</h4>

            {demoMode && <div className="alert alert-warning">Settings are disabled in demo mode.</div>}
            {error && <div className="alert alert-danger">{error}</div>}
            {!demoMode && !config && !error && <p className="text-muted">Loading…</p>}

            {config && (
              <>
                <div className="card border-secondary mb-4">
                  <div className="card-header">Systems</div>
                  <div className="card-body">
                    {ALL_SYSTEMS.map((system) => {
                      const actions = SYSTEM_ACTIONS[system];
                      return (
                        <div key={system} className="d-flex align-items-center gap-3 mb-2">
                          <div className="form-check mb-0">
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
                          {actions && (
                            <div className="d-flex gap-2 ms-auto">
                              {actions.map(({ label, endpoint }) => (
                                <UpdateButton key={endpoint} label={label} endpoint={endpoint} />
                              ))}
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>

                <div className="card border-secondary mb-4">
                  <div className="card-header">Server</div>
                  <div className="card-body">
                    <div className="mb-0">
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
                  <div className="card-header">Features</div>
                  <div className="card-body">
                    <div className="form-check mb-2">
                      <input
                        type="checkbox"
                        id="collections-enabled"
                        className="form-check-input"
                        checked={config.collections_enabled ?? true}
                        onChange={toggleCollections}
                      />
                      <label htmlFor="collections-enabled" className="form-check-label">
                        Enable collections
                        <small className="text-muted ms-2">Track owned cards across named collections</small>
                      </label>
                    </div>
                    <div className="form-check">
                      <input
                        type="checkbox"
                        id="pricing-enabled"
                        className="form-check-input"
                        checked={config.pricing_enabled ?? true}
                        onChange={togglePricing}
                      />
                      <label htmlFor="pricing-enabled" className="form-check-label">
                        Enable pricing
                        <small className="text-muted ms-2">Show market prices, purchase price inputs, and purchase history</small>
                      </label>
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
                  <button className="btn btn-primary" onClick={save} disabled={saving}>
                    {saving ? "Saving…" : "Save"}
                  </button>
                  {saved && <span className="text-success small">Saved. Some changes (e.g. port, paths) require a server restart.</span>}
                </div>
              </>
            )}
          </div>
      </main>
    </>
  );
}

export default function SettingsView() {
  return (
    <CollectionsProvider>
      <SystemTypeProvider>
        <SettingsContent />
      </SystemTypeProvider>
    </CollectionsProvider>
  );
}
