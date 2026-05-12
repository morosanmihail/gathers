import { createContext, useContext, useState, useEffect, useMemo, useCallback } from "react";
import { useOperations } from "../OperationsContext";

const SystemTypeContext = createContext({
  systemType: null,
  systems: [],
  selectedSearchSystem: null,
  setSelectedSearchSystem: () => {},
});

export function useSystemType() {
  return useContext(SystemTypeContext).systemType;
}

export function useSystems() {
  return useContext(SystemTypeContext).systems;
}

export function useSelectedSearchSystem() {
  const ctx = useContext(SystemTypeContext);
  return [ctx.selectedSearchSystem, ctx.setSelectedSearchSystem];
}

export function SystemTypeProvider({ children }) {
  const { fetch: opsFetch } = useOperations();
  const [systemType, setSystemType] = useState(null);
  const [systems, setSystems] = useState([]);
  const [selectedSearchSystem, setSelectedSearchSystem] = useState(null);

  useEffect(() => {
    opsFetch("Getting system info", null, "/system", {})
      .then((r) => {
        if (r && r.system) {
          setSystemType(r.system);
          const allSystems = r.systems && r.systems.length > 0 ? r.systems : [r.system];
          setSystems(allSystems);
          setSelectedSearchSystem(r.system);
        } else {
          setSystemType("MagicSQLite");
          setSystems(["MagicSQLite"]);
          setSelectedSearchSystem("MagicSQLite");
        }
      })
      .catch(() => {
        setSystemType("MagicSQLite");
        setSystems(["MagicSQLite"]);
        setSelectedSearchSystem("MagicSQLite");
      });
  }, [opsFetch]);

  const setSelectedSearchSystemStable = useCallback(setSelectedSearchSystem, []);
  const value = useMemo(
    () => ({ systemType, systems, selectedSearchSystem, setSelectedSearchSystem: setSelectedSearchSystemStable }),
    [systemType, systems, selectedSearchSystem, setSelectedSearchSystemStable]
  );

  return (
    <SystemTypeContext.Provider value={value}>
      {children}
    </SystemTypeContext.Provider>
  );
}
