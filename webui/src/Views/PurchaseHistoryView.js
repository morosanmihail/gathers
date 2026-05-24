import React, { useState, useEffect } from "react";
import { useParams, Link } from "react-router-dom";
import ViewProviders from "./ViewProviders";

function formatDate(iso) {
  const d = new Date(iso);
  if (isNaN(d)) return iso;
  return d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function PurchaseHistoryContent() {
  const { collection } = useParams();
  const [entries, setEntries] = useState(null);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (!collection) return;
    fetch(`/collection/cards/${encodeURIComponent(collection)}/purchase_history`)
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json();
      })
      .then((data) => setEntries(data.entries))
      .catch((e) => setError(e.message));
  }, [collection]);

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

      {entries && entries.length > 0 && (
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
              </tr>
            </thead>
            <tbody>
              {entries.map((e) => {
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
                  </tr>
                );
              })}
            </tbody>
            <tfoot className="table-light">
              <tr>
                <td colSpan={7} className="text-end text-muted small">Total spent</td>
                <td className="text-end fw-bold text-success">${totalSpent.toFixed(2)}</td>
                <td />
              </tr>
            </tfoot>
          </table>
        </div>
      )}
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
