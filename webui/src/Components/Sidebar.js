import React, { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import AddCollectionForm from "./AddCollectionForm";
import { useCollection, useCollections } from "./CollectionContext";
import OperationsTracker from "./CardListNavButtons/OperationsTracker";
import { useMode } from "../OperationsContext";

function useServerStatus() {
  const [status, setStatus] = useState({ ready: true, downloading: {}, demoMode: false });

  useEffect(() => {
    let timeout;
    const poll = () => {
      fetch("/api/system")
        .then((r) => (r.ok ? r.json() : null))
        .then((data) => {
          if (!data) {
            setStatus((s) => ({ ...s, ready: false, downloading: {} }));
            timeout = setTimeout(poll, 3000);
          } else if (data.downloading && Object.keys(data.downloading).length > 0) {
            setStatus({ ready: false, downloading: data.downloading, demoMode: !!data.demo_mode });
            timeout = setTimeout(poll, 1000);
          } else {
            setStatus({ ready: true, downloading: {}, demoMode: !!data.demo_mode });
          }
        })
        .catch(() => {
          setStatus((s) => ({ ...s, ready: false, downloading: {} }));
          timeout = setTimeout(poll, 3000);
        });
    };
    poll();
    return () => clearTimeout(timeout);
  }, []);

  return status;
}

export default function Sidebar() {
  const collection = useCollection();
  const collections = useCollections();
  const { mode, collectionsEnabled } = useMode();
  const isSearchOnly = mode === "search-only";
  const serverStatus = useServerStatus();
  const { demoMode } = serverStatus;

  return (
    <header>
      <nav id="sidebarMenu" className="d-lg-block sidebar bg-white">
        <nav className="navbar navbar-expand-lg navbar-light bg-light">
          <div className="container-fluid">
            <a className="navbar-brand" href="/">
              GatheRs
            </a>
          </div>
        </nav>
        <div className="position-sticky d-flex flex-column" style={{ height: "calc(100vh - 56px)" }}>
          <div
            className="nav flex-column nav-pills me-3"
            role="tablist"
            aria-orientation="vertical"
          >
            <Link to={"/search"} className="btn btn-secondary">
              Search
            </Link>
          </div>
          {!serverStatus.ready && (
            <>
              <hr />
              <div className="px-3 py-2 text-muted small">
                {Object.keys(serverStatus.downloading).length === 0 ? (
                  <div className="d-flex align-items-center gap-2">
                    <div className="spinner-border spinner-border-sm" role="status" aria-hidden="true" />
                    Server starting up…
                  </div>
                ) : (
                  Object.entries(serverStatus.downloading).map(([system, p]) => {
                    const pct = p.total > 0 ? Math.round((p.downloaded / p.total) * 100) : null;
                    const label = p.phase === "checking"
                      ? `Checking ${system}…`
                      : p.phase === "verifying"
                      ? `Verifying ${system}…`
                      : pct !== null
                      ? `Downloading ${system}: ${pct}%`
                      : `Downloading ${system}…`;
                    return (
                      <div key={system} className="mb-1">
                        <div className="d-flex align-items-center gap-2 mb-1">
                          <div className="spinner-border spinner-border-sm" role="status" aria-hidden="true" />
                          {label}
                        </div>
                        {p.phase === "downloading" && pct !== null && (
                          <div className="progress" style={{ height: "4px" }}>
                            <div
                              className="progress-bar"
                              role="progressbar"
                              style={{ width: `${pct}%` }}
                              aria-valuenow={pct}
                              aria-valuemin={0}
                              aria-valuemax={100}
                            />
                          </div>
                        )}
                      </div>
                    );
                  })
                )}
              </div>
            </>
          )}
          <hr />
          <div
            className="nav flex-column nav-pills me-3"
            role="tablist"
            aria-orientation="vertical"
          >
            {!isSearchOnly && collectionsEnabled && (
              <React.Fragment>
                {collections.map((c) => (
                  <Link
                    to={"/c/" + c.id + "/1"}
                    key={c.id}
                    className={"nav-link" + (c.id === collection ? " active" : "")}
                  >
                    {c.id}
                  </Link>
                ))}
                <hr />
                <AddCollectionForm />
                <hr />
                <OperationsTracker />
              </React.Fragment>
            )}
          </div>
          {!demoMode && (
            <div className="mt-auto px-0 pb-3">
              <Link to={"/settings"} className="btn btn-outline-secondary w-100">
                Settings
              </Link>
            </div>
          )}
        </div>
      </nav>
    </header>
  );
}
