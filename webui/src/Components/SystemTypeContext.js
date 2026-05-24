import { createContext, useContext, useState, useEffect, useMemo, useCallback } from "react";
import { useOperations } from "../OperationsContext";

const SystemTypeContext = createContext({
  systemType: null,
  systems: [],
  selectedSearchSystem: null,
  setSelectedSearchSystem: () => {},
  pricingEnabled: true,
  refreshSystemInfo: () => {},
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

export function usePricingEnabled() {
  return useContext(SystemTypeContext).pricingEnabled;
}

export function useRefreshSystemInfo() {
  return useContext(SystemTypeContext).refreshSystemInfo;
}

export function SystemTypeProvider({ children }) {
  const { fetch: opsFetch } = useOperations();
  const [systemType, setSystemType] = useState(null);
  const [systems, setSystems] = useState([]);
  const [selectedSearchSystem, setSelectedSearchSystem] = useState(null);
  const [pricingEnabled, setPricingEnabled] = useState(true);
  const [refreshTick, setRefreshTick] = useState(0);

  const refreshSystemInfo = useCallback(() => setRefreshTick((n) => n + 1), []);

  useEffect(() => {
    opsFetch("Getting system info", null, "/api/system", {})
      .then((r) => {
        if (r && r.system) {
          setSystemType(r.system);
          const allSystems = r.systems && r.systems.length > 0 ? r.systems : [r.system];
          setSystems(allSystems);
          setSelectedSearchSystem(r.system);
          setPricingEnabled(r.pricing_enabled !== false);
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
  }, [opsFetch, refreshTick]);

  const setSelectedSearchSystemStable = useCallback(setSelectedSearchSystem, []);
  const value = useMemo(
    () => ({ systemType, systems, selectedSearchSystem, setSelectedSearchSystem: setSelectedSearchSystemStable, pricingEnabled, refreshSystemInfo }),
    [systemType, systems, selectedSearchSystem, setSelectedSearchSystemStable, pricingEnabled, refreshSystemInfo]
  );

  return (
    <SystemTypeContext.Provider value={value}>
      {children}
    </SystemTypeContext.Provider>
  );
}
