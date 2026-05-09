import React, { useEffect } from "react";
import { useSearchParams } from "react-router-dom";
import SearchMagic from "./SearchMagic";
import SearchRiftbound from "./SearchRiftbound";
import SearchPokemon from "./SearchPokemon";
import { useSystems, useSelectedSearchSystem } from "./SystemTypeContext";

function systemLabel(system) {
  if (system === "RiftboundSQLite") return "Riftbound";
  if (system === "PokemonSQLite") return "Pokémon";
  if (system === "Scryfall") return "MTG (Scryfall)";
  return "MTG";
}

function Search({ startSearch = false, dedicatedPage = false, sidePanel = false }) {
  const systems = useSystems();
  const [selectedSystem, setSelectedSystem] = useSelectedSearchSystem();
  const [searchParams, setSearchParams] = useSearchParams();

  useEffect(() => {
    if (systems.length === 0) return;
    const urlSystem = searchParams.get("system");
    if (urlSystem && systems.includes(urlSystem)) {
      if (urlSystem !== selectedSystem) setSelectedSystem(urlSystem);
    } else if (selectedSystem) {
      setSearchParams((prev) => { prev.set("system", selectedSystem); return prev; }, { replace: true });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [systems, selectedSystem]);

  const renderSearch = () => {
    if (!selectedSystem) return null;
    if (selectedSystem === "RiftboundSQLite") {
      return (
        <SearchRiftbound
          startSearch={startSearch}
          dedicatedPage={dedicatedPage}
          sidePanel={sidePanel}
        />
      );
    }
    if (selectedSystem === "PokemonSQLite") {
      return (
        <SearchPokemon
          startSearch={startSearch}
          dedicatedPage={dedicatedPage}
          sidePanel={sidePanel}
        />
      );
    }
    return (
      <SearchMagic
        startSearch={startSearch}
        dedicatedPage={dedicatedPage}
        sidePanel={sidePanel}
      />
    );
  };

  return (
    <>
      {systems.length > 1 && (
        <div className="system-switcher btn-group mb-2" role="group">
          {systems.map((sys) => (
            <button
              key={sys}
              type="button"
              className={`btn btn-sm ${
                selectedSystem === sys ? "btn-primary" : "btn-outline-secondary"
              }`}
              onClick={() => {
                setSelectedSystem(sys);
                setSearchParams((prev) => { prev.set("system", sys); return prev; });
              }}
            >
              {systemLabel(sys)}
            </button>
          ))}
        </div>
      )}
      {renderSearch()}
    </>
  );
}

export default Search;
