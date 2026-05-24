import React, { useEffect } from "react";
import PokemonCard from "./PokemonCard";
import { useOperations } from "../OperationsContext";
import useCardSearch from "./useCardSearch";
import SearchPagination from "./SearchPagination";
import SortControls from "./SortControls";

const PAGE_SIZE = 24;

const ENERGY_TYPES = [
  "Fire", "Water", "Grass", "Lightning", "Psychic",
  "Fighting", "Darkness", "Metal", "Dragon", "Fairy", "Colorless",
];

const SORT_FIELDS = [
  { value: "Name",            label: "Name" },
  { value: "Rarity",         label: "Rarity" },
  { value: "SetCode",        label: "Set Code" },
  { value: "CollectorNumber",label: "Collector Number" },
];

function SearchPokemon({ startSearch = false, dedicatedPage = false, sidePanel = false }) {
  const ops = useOperations();

  const {
    cards, setCards,
    loading, setLoading,
    pageNumber,
    shouldSearch, setShouldSearch,
    searchOptions,
    handleSearchInput,
    handleArrayInput,
    handleMultiInput,
    handlePageChange,
    triggerSearch,
  } = useCardSearch({
    stringFields: ["name", "setCode", "collectorNumber", "sortBy", "sortOrder"],
    arrayFields: ["energyTypes"],
    startSearch,
    defaults: { sortBy: "Name", sortOrder: "Asc" },
  });

  useEffect(() => {
    if (!shouldSearch) return;
    setLoading(true);
    const url = `/api/pokemon/cards/search?limit=${PAGE_SIZE}&skip=${(pageNumber - 1) * PAGE_SIZE}`;
    ops
      .fetch("Searching", [], url, {
        method: "post",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify(searchOptions),
      })
      .then((data) => {
        setCards(data);
        setLoading(false);
        setShouldSearch(false);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pageNumber, shouldSearch]);

  return (
    <div
      className={dedicatedPage === true || sidePanel === true ? "" : "collapse"}
      id={dedicatedPage ? "main-search" : "search"}
    >
      <h2>Search</h2>
      <form onSubmit={(e) => { e.preventDefault(); triggerSearch(); }} className="list-group list-group-flush mx-3 mt-4">
        <div className="input-group">
          <input onChange={(e) => handleSearchInput(e, "name")} type="text" className="form-control" placeholder="Name" value={searchOptions.name} />
          <input onChange={(e) => handleSearchInput(e, "setCode")} className="form-control" placeholder="Set Code" value={searchOptions.setCode} />
        </div>
        <div className="input-group">
          <input onChange={(e) => handleSearchInput(e, "collectorNumber")} type="text" className="form-control" placeholder="Collector Number" value={searchOptions.collectorNumber} />
        </div>
        <div className="input-group">
          {ENERGY_TYPES.map((type) => (
            <div key={type} className="form-check form-check-inline">
              <input
                onChange={(e) => handleArrayInput("energyTypes", e)}
                className="form-check-input"
                type="checkbox"
                id={"energy-" + type}
                value={type}
                checked={searchOptions.energyTypes.includes(type)}
              />
              <label className="form-check-label" htmlFor={"energy-" + type}>{type}</label>
            </div>
          ))}
        </div>
        <div className="input-group">
          <SortControls
            sortBy={searchOptions.sortBy}
            sortOrder={searchOptions.sortOrder}
            fields={SORT_FIELDS}
            onChange={(field, order) => handleMultiInput({ sortBy: field, sortOrder: order }, { search: true })}
          />
        </div>
        <div className="input-group">
          <button onClick={triggerSearch} className="btn btn-outline-secondary" type="button">
            Search
          </button>
        </div>
        <div className="search-results" id="search-results">
          {loading ? (
            <p>Loading...</p>
          ) : (
            <div className="card-grid list">
              {cards.map((card) => (
                <PokemonCard
                  key={card.id + "-" + (card.details != null ? card.details.collectionId : "")}
                  id={card.id}
                  card={card}
                  details={card.details}
                  provider="PokemonSQLite"
                  showCollectionSelect={dedicatedPage && card.details == null}
                />
              ))}
            </div>
          )}
        </div>
        <SearchPagination cards={cards} pageSize={PAGE_SIZE} pageNumber={pageNumber} onPageChange={handlePageChange} />
      </form>
      <hr />
    </div>
  );
}

export default SearchPokemon;
