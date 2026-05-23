import { createContext, useContext, useState, useCallback } from "react";

const PricesContext = createContext({});
const FetchPricesContext = createContext(null);

export function PricesProvider({ children }) {
  const [prices, setPrices] = useState({});

  const fetchPrices = useCallback(async (ids) => {
    if (!ids || ids.length === 0) return;
    try {
      const params = new URLSearchParams(ids.map((id) => ["ids", id]));
      const res = await fetch(`/mtg/prices?${params.toString()}`);
      if (!res.ok) return;
      const data = await res.json();
      setPrices((prev) => ({ ...prev, ...data }));
    } catch (_) {
      // prices are best-effort
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
