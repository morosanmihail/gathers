import React, { createContext, useContext, useRef } from "react";
import { useOperations } from "../../OperationsContext";
import { useCardCache } from "./CardCacheContext";
import { useSystemType } from "../SystemTypeContext";

const CardLoaderContext = createContext(null);
export function useCardLoader() {
  return useContext(CardLoaderContext);
}


export function CardLoaderProvider({ children }) {
  const DataLoader = require("dataloader");
  const ops = useOperations();
  const [cache, dispatch] = useCardCache();
  const systemType = useSystemType();
  const systemTypeRef = useRef(systemType);
  systemTypeRef.current = systemType;

  function makeLoader(endpoint) {
    return new DataLoader(async (keys) => {
      const params = new URLSearchParams(keys.map((k) => ["ids", k]));
      const results = await ops
        .fetch(
          "Bulk updating details for cards",
          {},
          endpoint + "?" + params.toString(),
        );
      return keys.map((key) => results[key] || new Error(`No card for ${key}`));
    });
  }

  const mtgLoader = makeLoader("/api/mtg/cards");
  const riftboundLoader = makeLoader("/api/riftbound/cards");
  const pokemonLoader = makeLoader("/api/pokemon/cards");

  function getLoader(provider) {
    if (provider === "RiftboundSQLite") return riftboundLoader;
    if (provider === "PokemonSQLite") return pokemonLoader;
    return mtgLoader;
  }

  async function loadCard(id, provider) {
    const effectiveProvider = provider || systemTypeRef.current || "MagicSQLite";
    const cacheKey = effectiveProvider + ":" + id;
    if (cache[cacheKey]) {
      return cache[cacheKey];
    }
    const card = await getLoader(effectiveProvider).load(id);
    dispatch({ type: "ADD-TO-CACHE", id: cacheKey, data: card });
    return card;
  }

  return (
    <CardLoaderContext.Provider value={loadCard}>
      {children}
    </CardLoaderContext.Provider>
  );
}
