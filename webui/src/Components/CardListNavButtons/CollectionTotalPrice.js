import React, { useState, useEffect, useRef, useCallback } from "react";
import ReactDOM from "react-dom";
import { Link } from "react-router-dom";
import { useCollection } from "../CollectionContext";

function BreakdownTooltip({ pos, breakdown, onMouseEnter, onMouseLeave }) {
  const profitColor = breakdown.profit >= 0 ? "#4ade80" : "#f87171";
  return ReactDOM.createPortal(
    <div
      className="price-tooltip"
      style={{ top: pos.top, right: pos.right, minWidth: 200 }}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
    >
      <div className="price-tooltip-row">
        <span className="price-tooltip-retailer">Market value</span>
        <span className="price-tooltip-amounts">
          <span className="price-tooltip-normal">${breakdown.total_value.toFixed(2)}</span>
        </span>
      </div>
      <div className="price-tooltip-row">
        <span className="price-tooltip-retailer">Profit / loss</span>
        <span className="price-tooltip-amounts">
          <span style={{ color: profitColor, fontSize: "0.75rem" }}>
            {breakdown.profit >= 0 ? "+" : ""}${breakdown.profit.toFixed(2)}
          </span>
        </span>
      </div>
      {breakdown.untracked_value > 0 && (
        <div className="price-tooltip-row">
          <span className="price-tooltip-retailer" style={{ opacity: 0.7 }}>No purchase record</span>
          <span className="price-tooltip-amounts">
            <span className="price-tooltip-normal" style={{ opacity: 0.7 }}>${breakdown.untracked_value.toFixed(2)}</span>
          </span>
        </div>
      )}
      <div style={{ borderTop: "1px solid rgba(255,255,255,0.1)", marginTop: 4, paddingTop: 4, fontSize: "0.65rem", opacity: 0.5 }}>
        {breakdown.priced_count} of {breakdown.total_count} cards priced
      </div>
    </div>,
    document.body
  );
}

export default function CollectionTotalPrice() {
  const collection = useCollection();
  const [breakdown, setBreakdown] = useState(null);
  const [tick, setTick] = useState(0);
  const [tooltipPos, setTooltipPos] = useState(null);
  const badgeRef = useRef(null);
  const hideTimer = useRef(null);

  useEffect(() => {
    const handler = () => setTick((n) => n + 1);
    window.addEventListener("gathers:collection-updated", handler);
    return () => window.removeEventListener("gathers:collection-updated", handler);
  }, []);

  useEffect(() => {
    if (!collection) return;
    fetch(`/collection/cards/${encodeURIComponent(collection)}/value_breakdown`)
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => setBreakdown(d))
      .catch(() => setBreakdown(null));
  }, [collection, tick]);

  const showTooltip = useCallback(() => {
    clearTimeout(hideTimer.current);
    if (badgeRef.current) {
      const rect = badgeRef.current.getBoundingClientRect();
      setTooltipPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
    }
  }, []);

  const hideTooltip = useCallback(() => {
    hideTimer.current = setTimeout(() => setTooltipPos(null), 80);
  }, []);

  const cancelHide = useCallback(() => clearTimeout(hideTimer.current), []);

  if (!breakdown || breakdown.priced_count === 0) return null;

  return (
    <span className="d-flex align-items-center gap-2">
      <span
        ref={badgeRef}
        className="small text-muted"
        style={{ cursor: "default" }}
        onMouseEnter={showTooltip}
        onMouseLeave={hideTooltip}
      >
        Total: <strong className="text-success">${breakdown.total_value.toFixed(2)}</strong>
        <span className="ms-1 opacity-50">({breakdown.priced_count}/{breakdown.total_count})</span>
      </span>
      <Link
        to={`/c/${encodeURIComponent(collection)}/history`}
        className="btn btn-outline-secondary btn-sm"
        style={{ fontSize: "0.7rem", padding: "1px 6px" }}
      >
        History
      </Link>
      {tooltipPos && (
        <BreakdownTooltip
          pos={tooltipPos}
          breakdown={breakdown}
          onMouseEnter={cancelHide}
          onMouseLeave={hideTooltip}
        />
      )}
    </span>
  );
}
