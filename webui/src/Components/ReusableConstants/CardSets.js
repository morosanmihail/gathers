import React, { useState, useEffect, createContext, useContext } from "react";
import { useOperations } from "../../OperationsContext";
import { useSystems } from "../SystemTypeContext";

const MTG_SYSTEMS = ["MagicSQLite", "Scryfall"];

const CardSetsContext = createContext([]);
export function useCardSets() {
  return useContext(CardSetsContext);
}

export function CardSetsProvider({ children }) {
  const { fetch: opsFetch } = useOperations();
  const systems = useSystems();

  const [sets, setSets] = useState([]);

  useEffect(() => {
    if (!systems.some((s) => MTG_SYSTEMS.includes(s))) return;
    opsFetch("Getting all available sets", [], "/mtg/sets").then((data) => {
      setSets([{ code: "", name: "" }, ...data]);
    });
  }, [opsFetch, systems]);

  return (
    <CardSetsContext.Provider value={sets}>{children}</CardSetsContext.Provider>
  );
}
