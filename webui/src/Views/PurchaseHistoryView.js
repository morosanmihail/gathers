import React, { useState, useEffect } from "react";
import { useParams, Link } from "react-router-dom";
import ViewProviders from "./ViewProviders";
import { usePricingEnabled } from "../Components/SystemTypeContext";

function formatDate(iso) {
  const d = new Date(iso);
  if (isNaN(d)) return iso;
  return d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function EditRow({ entry, onSave, onCancel, saveError }) {
  const [qty, setQty] = useState(String(entry.quantity));
  const [foilQty, setFoilQty] = useState(String(entry.foil_quantity));
  const [normalPrice, setNormalPrice] = useState(entry.normal_price_per_unit != null ? String(entry.normal_price_per_unit) : "");
  const [foilPrice, setFoilPrice] = useState(entry.foil_price_per_unit != null ? String(entry.foil_price_per_unit) : "");

  const handleSave = () => {
    onSave({
      quantity: parseInt(qty) || 0,
      foil_quantity: parseInt(foilQty) || 0,
      normal_price_per_unit: normalPrice !== "" ? parseFloat(normalPrice) : null,
      foil_price_per_unit: foilPrice !== "" ? parseFloat(foilPrice) : null,
    });
  };

  const inputStyle = { width: 70, fontSize: "0.8rem" };

  return (
    <tr className="table-warning">
      <td className="text-muted" style={{ whiteSpace: "nowrap" }}>{formatDate(entry.recorded_at)}</td>
      <td className="fw-semibold" colSpan={saveError ? 2 : 1}>
        {entry.card_name ?? <span className="text-muted fst-italic">Unknown</span>}
        {saveError && <div className="text-danger small mt-1">{saveError}</div>}
      </td>
      {!saveError && <td className="text-muted small">{entry.set_code ?? "—"}</td>}
      <td className="text-end">
        <input type="number" min="0" className="form-control form-control-sm d-inline" style={inputStyle}
          value={qty} onChange={(e) => setQty(e.target.value)} />
      </td>
      <td className="text-end">
        <input type="number" min="0" className="form-control form-control-sm d-inline" style={inputStyle}
          value={foilQty} onChange={(e) => setFoilQty(e.target.value)} />
      </td>
      <td className="text-end">
        <input type="number" min="0" step="0.01" placeholder="—" className="form-control form-control-sm d-inline" style={inputStyle}
          value={normalPrice} onChange={(e) => setNormalPrice(e.target.value)} />
      </td>
      <td className="text-end">
        <input type="number" min="0" step="0.01" placeholder="—" className="form-control form-control-sm d-inline" style={inputStyle}
          value={foilPrice} onChange={(e) => setFoilPrice(e.target.value)} />
      </td>
      <td className="text-end" />
      <td className="text-muted small">{entry.provider || "—"}</td>
      <td>
        <div className="d-flex gap-1">
          <button className="btn btn-sm btn-success" style={{ padding: "1px 6px" }} onClick={handleSave}>✓</button>
          <button className="btn btn-sm btn-outline-secondary" style={{ padding: "1px 6px" }} onClick={onCancel}>✕</button>
        </div>
      </td>
    </tr>
  );
}

const PAGE_SIZE = 30;

function PurchaseHistoryContent() {
  const { collection } = useParams();
  const pricingEnabled = usePricingEnabled();
  const [entries, setEntries] = useState(null);
  const [error, setError] = useState(null);
  const [editingId, setEditingId] = useState(null);
  const [editError, setEditError] = useState(null);
  const [pendingDelete, setPendingDelete] = useState(null);
  const [page, setPage] = useState(1);

  const load = () => {
    if (!collection || !pricingEnabled) return;
    fetch(`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history`)
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json();
      })
      .then((data) => { setEntries(data.entries); setPage(1); })
      .catch((e) => setError(e.message));
  };

  useEffect(load, [collection, pricingEnabled]);

  const handleDelete = (entryId) => {
    fetch(`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history_entry/${entryId}`, {
      method: "DELETE",
    }).then((r) => {
      if (!r.ok && r.status !== 204) throw new Error(`HTTP ${r.status}`);
      setPendingDelete(null);
      load();
    }).catch((e) => setError(e.message));
  };

  const handleUpdate = (entryId, body) => {
    fetch(`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history_entry/${entryId}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }).then(async (r) => {
      if (r.status === 400) {
        const data = await r.json().catch(() => ({}));
        setEditError(data.error ?? "Invalid update.");
        return;
      }
      if (!r.ok && r.status !== 204) throw new Error(`HTTP ${r.status}`);
      setEditingId(null);
      setEditError(null);
      load();
    }).catch((e) => setError(e.message));
  };

  const totalSpent = entries
    ? entries.reduce((sum, e) => {
        const n = (e.normal_price_per_unit ?? 0) * e.quantity;
        const f = (e.foil_price_per_unit ?? 0) * e.foil_quantity;
        return sum + n + f;
      }, 0)
    : 0;

  return (
    <div className="container-fluid py-3" style={{ maxWidth: 1100 }}>
      <div className="d-flex align-items-center gap-3 mb-3">
        <Link to={`/c/${encodeURIComponent(collection)}/1`} className="btn btn-sm btn-outline-secondary">
          ← Back
        </Link>
        <h5 className="mb-0">
          Purchase History — <span className="text-success">{collection}</span>
        </h5>
        {entries && entries.length > 0 && (
          <span className="ms-auto small text-muted">
            Total spent: <strong className="text-success">${totalSpent.toFixed(2)}</strong>
            <span className="ms-2 opacity-50">({entries.length} records)</span>
          </span>
        )}
      </div>

      {!pricingEnabled && (
        <div className="alert alert-warning">
          Pricing is disabled. Enable it in <Link to="/settings">Settings</Link> to track purchase history.
        </div>
      )}

      {error && (
        <div className="alert alert-danger">{error}</div>
      )}

      {entries === null && !error && (
        <div className="text-muted text-center py-5">Loading…</div>
      )}

      {entries && entries.length === 0 && (
        <div className="text-muted text-center py-5">
          No purchase records for this collection yet.
          <br />
          <small>Add cards with a purchase price to start tracking.</small>
        </div>
      )}

      {pendingDelete != null && (
        <div className="alert alert-danger d-flex align-items-center gap-3">
          <span>Delete this purchase record?</span>
          <button className="btn btn-sm btn-danger" onClick={() => handleDelete(pendingDelete)}>Delete</button>
          <button className="btn btn-sm btn-outline-secondary" onClick={() => setPendingDelete(null)}>Cancel</button>
        </div>
      )}

      {entries && entries.length > 0 && (() => {
        const totalPages = Math.ceil(entries.length / PAGE_SIZE);
        const pageEntries = entries.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
        return (
        <div className="table-responsive">
          <table className="table table-sm table-striped table-hover align-middle">
            <thead className="table-light">
              <tr>
                <th>Date</th>
                <th>Card</th>
                <th>Set</th>
                <th className="text-end">Qty</th>
                <th className="text-end">Foil</th>
                <th className="text-end">Normal price</th>
                <th className="text-end">Foil price</th>
                <th className="text-end">Line total</th>
                <th>Provider</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {pageEntries.map((e) => {
                if (editingId === e.id) {
                  return (
                    <EditRow
                      key={e.id}
                      entry={e}
                      onSave={(body) => handleUpdate(e.id, body)}
                      onCancel={() => { setEditingId(null); setEditError(null); }}
                      saveError={editError}
                    />
                  );
                }
                const lineTotal =
                  (e.normal_price_per_unit ?? 0) * e.quantity +
                  (e.foil_price_per_unit ?? 0) * e.foil_quantity;
                return (
                  <tr key={e.id}>
                    <td className="text-muted" style={{ whiteSpace: "nowrap" }}>
                      {formatDate(e.recorded_at)}
                    </td>
                    <td className="fw-semibold">
                      {e.card_name ?? <span className="text-muted fst-italic">Unknown</span>}
                    </td>
                    <td className="text-muted small">{e.set_code ?? "—"}</td>
                    <td className="text-end">
                      {e.quantity > 0 ? (
                        <span className="badge bg-secondary">{e.quantity}</span>
                      ) : (
                        <span className="text-muted">—</span>
                      )}
                    </td>
                    <td className="text-end">
                      {e.foil_quantity > 0 ? (
                        <span className="badge" style={{ backgroundColor: "#7c3aed" }}>
                          {e.foil_quantity} ✦
                        </span>
                      ) : (
                        <span className="text-muted">—</span>
                      )}
                    </td>
                    <td className="text-end">
                      {e.normal_price_per_unit != null ? (
                        <span className="text-success fw-semibold">${e.normal_price_per_unit.toFixed(2)}</span>
                      ) : (
                        <span className="text-muted">—</span>
                      )}
                    </td>
                    <td className="text-end">
                      {e.foil_price_per_unit != null ? (
                        <span style={{ color: "#6f42c1", fontWeight: 600 }}>${e.foil_price_per_unit.toFixed(2)}</span>
                      ) : (
                        <span className="text-muted">—</span>
                      )}
                    </td>
                    <td className="text-end fw-semibold">
                      {lineTotal > 0 ? (
                        <span>${lineTotal.toFixed(2)}</span>
                      ) : (
                        <span className="text-muted">—</span>
                      )}
                    </td>
                    <td className="text-muted small">{e.provider || "—"}</td>
                    <td>
                      <div className="d-flex gap-1">
                        <button
                          className="btn btn-sm btn-outline-primary"
                          style={{ padding: "1px 6px", fontSize: "0.75rem" }}
                          onClick={() => { setEditingId(e.id); setEditError(null); setPendingDelete(null); }}
                        >✎</button>
                        <button
                          className="btn btn-sm btn-outline-danger"
                          style={{ padding: "1px 6px", fontSize: "0.75rem" }}
                          onClick={() => { setPendingDelete(e.id); setEditingId(null); }}
                        >✕</button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
            <tfoot className="table-light">
              <tr>
                <td colSpan={7} className="text-end text-muted small">Total spent</td>
                <td className="text-end fw-bold text-success">${totalSpent.toFixed(2)}</td>
                <td /><td />
              </tr>
            </tfoot>
          </table>
          {totalPages > 1 && (
            <nav className="d-flex align-items-center justify-content-center gap-2 mt-2">
              <button
                className="btn btn-sm btn-outline-secondary"
                disabled={page === 1}
                onClick={() => setPage(p => p - 1)}
              >‹ Prev</button>
              <span className="small text-muted">Page {page} of {totalPages}</span>
              <button
                className="btn btn-sm btn-outline-secondary"
                disabled={page === totalPages}
                onClick={() => setPage(p => p + 1)}
              >Next ›</button>
            </nav>
          )}
        </div>
        );
      })()}
    </div>
  );
}

export default function PurchaseHistoryView() {
  return (
    <ViewProviders>
      <PurchaseHistoryContent />
    </ViewProviders>
  );
}
