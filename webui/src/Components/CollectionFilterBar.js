import React, { useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useSystems } from "./SystemTypeContext";
import SortControls from "./SortControls";
import "mana-font/css/mana.min.css";

const MTG_COLOURS = [
  { value: "White", mana: "w", bg: "#f9faf4", border: "#c4b78a" },
  { value: "Blue",  mana: "u", bg: "#0e68ab", border: "#0a4f82" },
  { value: "Black", mana: "b", bg: "#2a2a2a", border: "#555" },
  { value: "Red",   mana: "r", bg: "#d3202a", border: "#a01820" },
  { value: "Green", mana: "g", bg: "#00733e", border: "#005a30" },
];

const RB_DOMAINS = [
  { value: "Calm",   label: "Ca", bg: "#3a7abf", border: "#2a5a8f", title: "Calm" },
  { value: "Chaos",  label: "Ch", bg: "#d4681a", border: "#a34e12", title: "Chaos" },
  { value: "Fury",   label: "Fu", bg: "#b02020", border: "#801818", title: "Fury" },
  { value: "Mind",   label: "Mi", bg: "#7b3fa0", border: "#5a2e78", title: "Mind" },
  { value: "Body",   label: "Bo", bg: "#2e8b45", border: "#1e6030", title: "Body" },
  { value: "Order",  label: "Or", bg: "#b89a20", border: "#8a7010", title: "Order" },
];

const POKEMON_ENERGY = [
  { value: "Fire",       label: "🔥", bg: "#e8401c", border: "#b02e10" },
  { value: "Water",      label: "💧", bg: "#3a8bdc", border: "#2060a8" },
  { value: "Grass",      label: "🌿", bg: "#3fa84a", border: "#2a7034" },
  { value: "Lightning",  label: "⚡", bg: "#e8c020", border: "#b09010" },
  { value: "Psychic",    label: "🔮", bg: "#c040b0", border: "#8c2c80" },
  { value: "Fighting",   label: "👊", bg: "#c06030", border: "#8a4020" },
  { value: "Darkness",   label: "🌑", bg: "#303050", border: "#181828" },
  { value: "Metal",      label: "⚙", bg: "#8090a0", border: "#505a60" },
  { value: "Dragon",     label: "🐉", bg: "#6050c0", border: "#403590" },
  { value: "Fairy",      label: "✨", bg: "#e060a0", border: "#a84070" },
  { value: "Colorless",  label: "◇", bg: "#a0a0a0", border: "#606060" },
];

const SORT_FIELDS = [
  { value: "Name",             label: "Name" },
  { value: "SetCode",          label: "Set" },
  { value: "Rarity",           label: "Rarity" },
  { value: "CollectorNumber",  label: "No." },
  { value: "Artist",           label: "Artist" },
];

const SYSTEM_LABELS = {
  MagicSQLite: "MTG",
  RiftboundSQLite: "Riftbound",
  PokemonSQLite: "Pokémon",
  Scryfall: "Scryfall",
};

function readFilters(searchParams) {
  const storedViewMode = localStorage.getItem("cf_viewMode") ?? "grid";
  return {
    name:            searchParams.get("cf_name") ?? "",
    setCode:         searchParams.get("cf_setCode") ?? "",
    rarity:          searchParams.get("cf_rarity") ?? "",
    artist:          searchParams.get("cf_artist") ?? "",
    text:            searchParams.get("cf_text") ?? "",
    provider:        searchParams.get("cf_provider") ?? "",
    sortBy:          searchParams.get("cf_sortBy") ?? "Name",
    sortOrder:       searchParams.get("cf_sortOrder") ?? "Asc",
    viewMode:        searchParams.get("cf_viewMode") ?? storedViewMode,
    colorIdentities: searchParams.getAll("cf_color"),
    domains:         searchParams.getAll("cf_domain"),
    energyTypes:     searchParams.getAll("cf_energy"),
  };
}

export function useCollectionFilters() {
  const [searchParams] = useSearchParams();
  return readFilters(searchParams);
}

export function collectionFiltersActive(filters) {
  return (
    filters.name !== "" ||
    filters.setCode !== "" ||
    filters.rarity !== "" ||
    filters.artist !== "" ||
    filters.text !== "" ||
    filters.colorIdentities.length > 0 ||
    filters.domains.length > 0 ||
    filters.energyTypes.length > 0
  );
}

export default function CollectionFilterBar() {
  const systems = useSystems();
  const [searchParams, setSearchParams] = useSearchParams();
  const [open, setOpen] = useState(false);
  const filters = readFilters(searchParams);

  const setFilter = (key, value) => {
    const next = new URLSearchParams(searchParams);
    if (value === "" || value === null) {
      next.delete(key);
    } else {
      next.set(key, value);
    }
    next.set("page", "1");
    setSearchParams(next);
  };

  const setArrayFilter = (key, value, checked) => {
    const next = new URLSearchParams(searchParams);
    const existing = next.getAll(key).filter((v) => v !== value);
    next.delete(key);
    const updated = checked ? [...existing, value] : existing;
    updated.forEach((v) => next.append(key, v));
    next.set("page", "1");
    setSearchParams(next);
  };

  const clearFilters = () => {
    const next = new URLSearchParams(searchParams);
    ["cf_name","cf_setCode","cf_rarity","cf_artist","cf_text","cf_provider",
     "cf_sortBy","cf_sortOrder","cf_color","cf_domain","cf_energy","page"].forEach((k) => next.delete(k));
    setSearchParams(next);
  };

  const hasActive = collectionFiltersActive(filters);

  return (
    <div className="collection-filter-bar bg-body-tertiary border-bottom" data-bs-theme="dark">
      <div className="d-flex align-items-center gap-2 px-3 py-2">
        <button
          className={`btn btn-sm ${open ? "btn-secondary" : "btn-outline-secondary"}`}
          onClick={() => setOpen((o) => !o)}
          type="button"
        >
          Filters {hasActive && <span className="badge bg-primary ms-1">●</span>}
        </button>

        {hasActive && (
          <button className="btn btn-sm btn-outline-danger" onClick={clearFilters} type="button">
            Clear
          </button>
        )}

        <div className="ms-auto">
          <div className="btn-group btn-group-sm" role="group">
            <button
              type="button"
              className={`btn ${filters.viewMode === "grid" ? "btn-secondary" : "btn-outline-secondary"}`}
              onClick={() => { setFilter("cf_viewMode", "grid"); localStorage.setItem("cf_viewMode", "grid"); }}
              title="Grid view"
            >
              ⊞
            </button>
            <button
              type="button"
              className={`btn ${filters.viewMode === "list" ? "btn-secondary" : "btn-outline-secondary"}`}
              onClick={() => { setFilter("cf_viewMode", "list"); localStorage.setItem("cf_viewMode", "list"); }}
              title="List view"
            >
              ☰
            </button>
          </div>
        </div>
      </div>

      {open && (
        <div className="px-3 pb-3">
          <div className="row g-2 mb-2">
            <div className="col-sm-4">
              <input
                type="text"
                className="form-control form-control-sm"
                placeholder="Name"
                value={filters.name}
                onChange={(e) => setFilter("cf_name", e.target.value)}
              />
            </div>
            <div className="col-sm-2">
              <input
                type="text"
                className="form-control form-control-sm"
                placeholder="Set Code"
                value={filters.setCode}
                onChange={(e) => setFilter("cf_setCode", e.target.value)}
              />
            </div>
            <div className="col-sm-2">
              <select
                className="form-select form-select-sm"
                value={filters.rarity}
                onChange={(e) => setFilter("cf_rarity", e.target.value)}
              >
                <option value="">Any Rarity</option>
                <option value="Common">Common</option>
                <option value="Uncommon">Uncommon</option>
                <option value="Rare">Rare</option>
                <option value="Mythic">Mythic</option>
              </select>
            </div>
            <div className="col-sm-2">
              <input
                type="text"
                className="form-control form-control-sm"
                placeholder="Artist"
                value={filters.artist}
                onChange={(e) => setFilter("cf_artist", e.target.value)}
              />
            </div>
            <div className="col-sm-2">
              <input
                type="text"
                className="form-control form-control-sm"
                placeholder="Card Text"
                value={filters.text}
                onChange={(e) => setFilter("cf_text", e.target.value)}
              />
            </div>
          </div>

          <div className="row g-2 mb-2">
            {systems.length > 1 && (
              <div className="col-sm-3">
                <select
                  className="form-select form-select-sm"
                  value={filters.provider}
                  onChange={(e) => setFilter("cf_provider", e.target.value)}
                >
                  <option value="">All Games</option>
                  {systems.map((s) => (
                    <option key={s} value={s}>{SYSTEM_LABELS[s] ?? s}</option>
                  ))}
                </select>
              </div>
            )}

            {(systems.includes("MagicSQLite") || systems.includes("Scryfall")) && (
              <div className="col-auto d-flex align-items-center gap-1">
                <small className="text-muted me-1">Colours:</small>
                {MTG_COLOURS.map(({ value, mana, bg, border }) => {
                  const active = filters.colorIdentities.includes(value);
                  return (
                    <button
                      key={value}
                      type="button"
                      title={value}
                      onClick={() => setArrayFilter("cf_color", value, !active)}
                      className="mana-toggle-btn"
                      style={{
                        background: active ? bg : "transparent",
                        borderColor: active ? border : "rgba(255,255,255,0.3)",
                        opacity: active ? 1 : 0.45,
                        transform: active ? "scale(1.15)" : "scale(1)",
                      }}
                    >
                      <i className={`ms ms-${mana} ms-cost`} />
                    </button>
                  );
                })}
              </div>
            )}

            {systems.includes("RiftboundSQLite") && (
              <div className="col-auto d-flex align-items-center gap-1">
                <small className="text-muted me-1">Domains:</small>
                {RB_DOMAINS.map(({ value, label, bg, border, title }) => {
                  const active = filters.domains.includes(value);
                  return (
                    <button
                      key={value}
                      type="button"
                      title={title}
                      onClick={() => setArrayFilter("cf_domain", value, !active)}
                      className="mana-toggle-btn"
                      style={{
                        background: active ? bg : "transparent",
                        borderColor: active ? border : "rgba(255,255,255,0.3)",
                        opacity: active ? 1 : 0.45,
                        transform: active ? "scale(1.15)" : "scale(1)",
                        fontSize: "0.65rem",
                        fontWeight: "700",
                        color: active ? "#fff" : "rgba(255,255,255,0.7)",
                        letterSpacing: "-0.5px",
                      }}
                    >
                      {label}
                    </button>
                  );
                })}
              </div>
            )}
          </div>

          {systems.includes("PokemonSQLite") && (
          <div className="row g-2 mb-2">
            <div className="col-auto d-flex align-items-center gap-1 flex-wrap">
              <small className="text-muted me-1">Energy:</small>
              {POKEMON_ENERGY.map(({ value, label, bg, border }) => {
                const active = filters.energyTypes.includes(value);
                return (
                  <button
                    key={value}
                    type="button"
                    title={value}
                    onClick={() => setArrayFilter("cf_energy", value, !active)}
                    className="mana-toggle-btn"
                    style={{
                      background: active ? bg : "transparent",
                      borderColor: active ? border : "rgba(255,255,255,0.3)",
                      opacity: active ? 1 : 0.45,
                      transform: active ? "scale(1.15)" : "scale(1)",
                      fontSize: "1rem",
                      lineHeight: 1,
                    }}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          </div>
          )}

          {filters.viewMode !== "list" && (
            <div className="row g-2">
              <div className="col-auto">
                <SortControls
                  sortBy={filters.sortBy}
                  sortOrder={filters.sortOrder}
                  fields={SORT_FIELDS}
                  onChange={(field, order) => {
                    const next = new URLSearchParams(searchParams);
                    next.set("cf_sortBy", field);
                    next.set("cf_sortOrder", order);
                    next.set("page", "1");
                    setSearchParams(next);
                  }}
                />
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
