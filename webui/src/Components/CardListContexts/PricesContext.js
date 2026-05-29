import { createContext, useContext, useState, useCallback } from "react";

const PricesContext = createContext({});
const FetchPricesContext = createContext(null);

export function PricesProvider({ children }) {
  const [prices, setPrices] = useState({});

  const fetchPrices = useCallback(async ({ mtgIds = [], pokemonIds = [] } = {}) => {
    const fetches = [];

    if (mtgIds.length > 0) {
      fetches.push(
        fetch(`/api/mtg/prices?${new URLSearchParams(mtgIds.map((id) => ["ids", id]))}`)
          .then((res) => (res.ok ? res.json() : null))
          .catch(() => null)
      );
    }

    if (pokemonIds.length > 0) {
      fetches.push(
        fetch(`/api/pokemon/prices?${new URLSearchParams(pokemonIds.map((id) => ["ids", id]))}`)
          .then((res) => (res.ok ? res.json() : null))
          .catch(() => null)
      );
    }

    if (fetches.length === 0) return;

    const results = await Promise.all(fetches);
    const merged = Object.assign({}, ...results.filter(Boolean));
    if (Object.keys(merged).length > 0) {
      setPrices((prev) => ({ ...prev, ...merged }));
    }
  }, []);

  return (
    <PricesContext.Provider value={prices}>
      <FetchPricesContext.Provider value={fetchPrices}>
        {children}
      </FetchPricesContext.Provider>
    </PricesContext.Provider>
  );
}

export function usePrices() {
  return useContext(PricesContext);
}

export function useFetchPrices() {
  return useContext(FetchPricesContext);
}
