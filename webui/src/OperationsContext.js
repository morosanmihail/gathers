import { createContext, useState, useContext, useCallback } from "react";

export const ModeContext = createContext({ mode: "full", collectionsEnabled: false });

export function ModeProvider({ children, mode = "full", collectionsEnabled = false }) {
  return (
    <ModeContext.Provider value={{ mode, collectionsEnabled }}>{children}</ModeContext.Provider>
  );
}

export function useMode() {
  return useContext(ModeContext);
}

export const OperationsContext = createContext({});

export function OperationsProvider({ children }) {
  const [operations, setOperations] = useState({});

  const opsFetch = useCallback(async (message, defaultValue, ...args) => {
    const opId = crypto.randomUUID?.() ?? Math.random().toString(36).slice(2);
    setOperations((prev) => ({ ...prev, [opId]: { message } }));
    const removeOp = () => setOperations((prev) => { const copy = { ...prev }; delete copy[opId]; return copy; });
    try {
      const response = await fetch(...args);
      removeOp();
      if (response.ok) {
        return await response.json();
      }
      return defaultValue;
    } catch (e) {
      removeOp();
      return defaultValue;
    }
  }, []);

  return (
    <OperationsContext.Provider
      value={{ operations: operations, fetch: opsFetch }}
    >
      {children}
    </OperationsContext.Provider>
  );
}

export function useOperations() {
  return useContext(OperationsContext);
}
