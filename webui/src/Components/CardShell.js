import React, { useState, useEffect, useRef, useCallback } from "react";
import ReactDOM from "react-dom";
import { Link } from "react-router-dom";
import CardDetails from "./CardDetails";
import { useSelectedCardsDispatch } from "./CardListContexts/SelectedCardsContext";
import { useCardLoader } from "./CardListContexts/CardLoaderContext";
import { usePrices } from "./CardListContexts/PricesContext";
import { useOperations } from "../OperationsContext";
import { useCardsDispatch } from "./CardListContexts/CardsContext";
import { useRefreshCardList } from "./CardListContexts/RefreshCardListContext";
import { usePricingEnabled } from "./SystemTypeContext";

function ProviderIcon({ provider }) {
  if (provider?.includes("Riftbound")) return <span className="card-list-provider-icon" title="Riftbound">⚡</span>;
  if (provider?.includes("Pokemon")) return <span className="card-list-provider-icon" title="Pokémon">🔴</span>;
  return <span className="card-list-provider-icon" title="Magic: The Gathering">🎴</span>;
}

function getArtist(card) {
  if (Array.isArray(card.artists)) return card.artists.join(", ");
  return card.artist ?? "";
}

function bestPrice(cardPrices) {
  if (!cardPrices?.paper) return null;
  const normals = Object.values(cardPrices.paper)
    .map((r) => r.normal)
    .filter((p) => p != null);
  if (normals.length > 0) return { value: Math.min(...normals), type: "normal" };
  const foils = Object.values(cardPrices.paper)
    .map((r) => r.foil)
    .filter((p) => p != null);
  if (foils.length > 0) return { value: Math.min(...foils), type: "foil" };
  return null;
}

function PriceTooltip({ pos, cardPrices, onMouseEnter, onMouseLeave }) {
  return ReactDOM.createPortal(
    <div
      className="price-tooltip"
      style={{ top: pos.top, right: pos.right }}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onClick={(e) => e.stopPropagation()}
    >
      {Object.entries(cardPrices.paper).map(([retailer, rp]) => (
        <div key={retailer} className="price-tooltip-row">
          <span className="price-tooltip-retailer">{retailer}</span>
          <span className="price-tooltip-amounts">
            {rp.normal != null && (
              <span className="price-tooltip-normal">${rp.normal.toFixed(2)}</span>
            )}
            {rp.foil != null && (
              <span className="price-tooltip-foil">${rp.foil.toFixed(2)} ✦</span>
            )}
          </span>
        </div>
      ))}
    </div>,
    document.body
  );
}

function preferredRetailerPrices(cardPrices) {
  if (!cardPrices?.paper) return null;
  const rp = Object.entries(cardPrices.paper).find(([k]) => k.toLowerCase() === "cardmarket")?.[1]
    ?? cardPrices.paper["raw"]
    ?? Object.values(cardPrices.paper)[0];
  if (!rp || (rp.normal == null && rp.foil == null)) return null;
  return { normal: rp.normal ?? null, foil: rp.foil ?? null };
}

function PriceCell({ uuid }) {
  const pricingEnabled = usePricingEnabled();
  const prices = usePrices();
  const cardPrices = prices[uuid];
  const [tooltipPos, setTooltipPos] = useState(null);
  const cellRef = useRef(null);
  const hideTimer = useRef(null);

  const showTooltip = useCallback(() => {
    clearTimeout(hideTimer.current);
    if (cellRef.current) {
      const rect = cellRef.current.getBoundingClientRect();
      setTooltipPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
    }
  }, []);

  const hideTooltip = useCallback(() => {
    hideTimer.current = setTimeout(() => setTooltipPos(null), 80);
  }, []);

  const cancelHide = useCallback(() => clearTimeout(hideTimer.current), []);

  if (!pricingEnabled) return <span className="card-list-price" />;
  const rp = preferredRetailerPrices(cardPrices);
  if (!rp) return <span className="card-list-price" />;

  return (
    <span
      className="card-list-price"
      ref={cellRef}
      style={{ display: "flex", flexDirection: "row", gap: 4, flexWrap: "wrap", alignItems: "center" }}
      onMouseEnter={showTooltip}
      onMouseLeave={hideTooltip}
    >
      {rp.normal != null && (
        <span className="price-badge">${rp.normal.toFixed(2)}</span>
      )}
      {rp.foil != null && (
        <span className="price-badge price-badge-foil">${rp.foil.toFixed(2)} ✦</span>
      )}
      {tooltipPos && (
        <PriceTooltip
          pos={tooltipPos}
          cardPrices={cardPrices}
          onMouseEnter={cancelHide}
          onMouseLeave={hideTooltip}
        />
      )}
    </span>
  );
}

function PurchaseHistoryTooltip({ pos, entries, onMouseEnter, onMouseLeave }) {
  return ReactDOM.createPortal(
    <div
      className="price-tooltip"
      style={{ top: pos.top, right: pos.right, minWidth: 180 }}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onClick={(e) => e.stopPropagation()}
    >
      <div style={{ fontSize: "0.7rem", fontWeight: 600, marginBottom: 4, opacity: 0.7 }}>Purchase history</div>
      {entries.map((e) => (
        <div key={e.id} className="price-tooltip-row">
          <span className="price-tooltip-retailer" style={{ fontSize: "0.68rem" }}>
            {e.recorded_at.slice(0, 10)}
            {e.quantity > 0 && ` ×${e.quantity}`}
            {e.foil_quantity > 0 && ` ✦×${e.foil_quantity}`}
          </span>
          <span className="price-tooltip-amounts" style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", gap: 1 }}>
            {e.normal_price_per_unit != null && (
              <span className="price-tooltip-normal">${e.normal_price_per_unit.toFixed(2)}</span>
            )}
            {e.foil_price_per_unit != null && (
              <span className="price-tooltip-foil">${e.foil_price_per_unit.toFixed(2)} ✦</span>
            )}
            {e.normal_price_per_unit == null && e.foil_price_per_unit == null && (
              <span style={{ opacity: 0.5, fontSize: "0.68rem" }}>—</span>
            )}
          </span>
        </div>
      ))}
    </div>,
    document.body
  );
}

function PurchaseHistoryBadge({ collectionId, cardUuid }) {
  const pricingEnabled = usePricingEnabled();
  const [entries, setEntries] = useState(null);
  const [tooltipPos, setTooltipPos] = useState(null);
  const badgeRef = useRef(null);
  const hideTimer = useRef(null);

  useEffect(() => {
    if (!pricingEnabled || !collectionId || !cardUuid) return;
    fetch(
      `/api/collection/cards/${encodeURIComponent(collectionId)}/purchase_history/${encodeURIComponent(cardUuid)}`
    )
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => setEntries(data?.entries ?? []))
      .catch(() => {});
  }, [pricingEnabled, collectionId, cardUuid]);

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

  if (!pricingEnabled) return null;
  if (!entries || entries.length === 0) return null;

  const latest = entries[0];
  const latestPrice = latest.normal_price_per_unit ?? latest.foil_price_per_unit;
  const latestIsFoil = latest.normal_price_per_unit == null && latest.foil_price_per_unit != null;
  return (
    <span style={{ display: "inline-flex", alignItems: "center" }}>
      <span
        ref={badgeRef}
        className="badge"
        style={{ fontSize: "0.65rem", background: "rgba(124,58,237,0.75)", cursor: "default" }}
        onMouseEnter={showTooltip}
        onMouseLeave={hideTooltip}
      >
        {latestPrice != null
          ? `$${latestPrice.toFixed(2)}${latestIsFoil ? " ✦" : ""} paid`
          : "no price"}
      </span>
      {tooltipPos && entries.length > 0 && (
        <PurchaseHistoryTooltip
          pos={tooltipPos}
          entries={entries}
          onMouseEnter={cancelHide}
          onMouseLeave={hideTooltip}
        />
      )}
    </span>
  );
}

function QtyActionCell({ id, details, foil }) {
  const pricingEnabled = usePricingEnabled();
  const ops = useOperations();
  const cardsDispatch = useCardsDispatch();
  const triggerRefresh = useRefreshCardList();
  const prices = usePrices();
  const [priceInput, setPriceInput] = useState("");

  const cardPrices = prices[id];
  useEffect(() => {
    if (!cardPrices?.paper) return;
    const rp = Object.entries(cardPrices.paper).find(([k]) => k.toLowerCase() === "cardmarket")?.[1]
      ?? Object.values(cardPrices.paper)[0];
    const p = foil ? (rp?.foil ?? rp?.normal) : (rp?.normal ?? rp?.foil);
    if (p != null && priceInput === "") setPriceInput(p.toFixed(2));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cardPrices]);

  if (!details) return <span className="card-list-qty-actions" />;

  const qty = foil ? details.foilQuantity : details.quantity;

  const mutate = (delta, priceVal) => {
    const add = delta > 0;
    const url = `/api/collection/cards/${encodeURIComponent(details.collectionId)}/${add ? "add" : "delete"}`;
    const parsedPrice = parseFloat(priceVal);
    const body = {
      id,
      collectionId: details.collectionId,
      quantity: foil ? 0 : Math.abs(delta),
      foilQuantity: foil ? Math.abs(delta) : 0,
      ...(add && !isNaN(parsedPrice) && parsedPrice > 0 ? { purchasePrice: parsedPrice } : {}),
    };
    ops.fetch("Updating " + id, {}, url, {
      method: "post",
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }).then((data) => {
      cardsDispatch({ type: "added", card: add ? data[0] : data });
      triggerRefresh(true);
    }).catch(() => {});
  };

  return (
    <span className="card-list-qty-actions" onClick={(e) => e.stopPropagation()}>
      <span className={`badge ${foil ? "bg-info text-dark" : "bg-secondary"}`}>×{qty}</span>
      {pricingEnabled && (
        <input
          type="number"
          min="0"
          step="0.01"
          value={priceInput}
          placeholder="$"
          onChange={(e) => setPriceInput(e.target.value)}
          style={{ width: 54, fontSize: "0.72rem", padding: "1px 3px" }}
          className="form-control form-control-sm"
        />
      )}
      <button
        className="btn btn-sm btn-outline-success"
        style={{ padding: "1px 5px", fontSize: "0.75rem" }}
        onClick={() => mutate(1, pricingEnabled ? priceInput : "")}
      >+</button>
      <button
        className="btn btn-sm btn-outline-danger"
        style={{ padding: "1px 5px", fontSize: "0.75rem" }}
        onClick={() => mutate(-1, "")}
      >−</button>
    </span>
  );
}

export default function CardShell({ id, card = null, details = null, provider = null, detailPath, getImagePath, showCollectionSelect = false, listMode = false }) {
  const pricingEnabled = usePricingEnabled();
  const [_card, setCard] = useState(card);
  const [loadFailed, setLoadFailed] = useState(false);
  const [selected, setSelected] = useState(false);

  const selectedDispatch = useSelectedCardsDispatch();
  const loader = useCardLoader();

  const toggleSelected = () => {
    if (details != null) {
      selectedDispatch({ type: !selected ? "added" : "deleted", card: details });
      setSelected((s) => !s);
    }
  };

  useEffect(() => {
    if (_card == null) {
      loader(id, provider).then(setCard).catch(() => setLoadFailed(true));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, _card, details, provider]);

  if (loadFailed) return null;

  const imagePath = _card != null ? getImagePath(_card) : "";

  if (listMode) {
    const effectiveProvider = details?.provider || provider;
    return (
      <div
        className={"card-list-item" + (selected ? " selected" : "")}
        onClick={toggleSelected}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && toggleSelected()}
      >
        {_card == null ? (
          <span className="text-muted">Loading…</span>
        ) : (
          <>
            <ProviderIcon provider={effectiveProvider} />
            <span className="card-list-name">
              <Link to={detailPath} onClick={(e) => e.stopPropagation()}>{_card.name}</Link>
            </span>
            <span className="card-list-set text-muted">
              {_card.setCode}
              {_card.collectorNumber ? <span className="card-list-collector-num"> #{_card.collectorNumber}</span> : null}
            </span>
            <span className="card-list-rarity text-muted">{_card.rarity ?? ""}</span>
            <span className="card-list-artist text-muted">{getArtist(_card)}</span>
            {pricingEnabled && <PriceCell uuid={id} />}
            <QtyActionCell id={id} details={details} foil={false} />
            <QtyActionCell id={id} details={details} foil={true} />
            {pricingEnabled && (
              <span className="card-list-history" onClick={(e) => e.stopPropagation()}>
                {details != null && (
                  <PurchaseHistoryBadge collectionId={details.collectionId} cardUuid={id} />
                )}
              </span>
            )}
          </>
        )}
      </div>
    );
  }

  return (
    <>
      {_card == null ? (
        <p>Loading...</p>
      ) : (
        <div className={"card" + (selected ? " border border-primary" : "")}>
          <img src={imagePath} alt={_card.name} loading="lazy" />
          <CardDetails
            id={id}
            details={details}
            toggleSelected={toggleSelected}
            showCollectionSelect={showCollectionSelect}
          />
          <div className="card-info">
            <div className="row align-items-center">
              <span className="col-sm-8">
                <Link to={detailPath}>{_card.name}</Link>
                {details != null ? (
                  <span className="badge bg-secondary">{details.collectionId}</span>
                ) : (
                  ""
                )}
              </span>
              <span className="col-sm-11">{_card.setCode}</span>
              {details != null && (
                <span className="col-sm-11 mt-1">
                  <PurchaseHistoryBadge
                    collectionId={details.collectionId}
                    cardUuid={id}
                  />
                </span>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
