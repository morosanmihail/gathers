import React, { useState } from "react";
import { useCollection, useCollections } from "./CollectionContext";
import { useOperations, useMode } from "../OperationsContext";
import { useCardsDispatch } from "../Components/CardListContexts/CardsContext";
import { useRefreshCardList } from "./CardListContexts/RefreshCardListContext";
import { usePrices } from "./CardListContexts/PricesContext";

const ROW_STYLE = {
  background: "rgba(0,0,0,0.45)",
  borderRadius: 4,
  padding: "2px 4px",
  backdropFilter: "blur(2px)",
};

function PriceInput({ value, onChange }) {
  return (
    <input
      type="number"
      min="0"
      step="0.01"
      className="form-control form-control-sm"
      style={{ width: 60, fontSize: "0.72rem", padding: "1px 3px" }}
      value={value}
      placeholder="$"
      onClick={(e) => e.stopPropagation()}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

function preferredPrices(cardPrices) {
  if (!cardPrices?.paper) return null;
  const rp = Object.entries(cardPrices.paper).find(([k]) => k.toLowerCase() === "cardmarket")?.[1]
    ?? Object.values(cardPrices.paper)[0];
  if (!rp || (rp.normal == null && rp.foil == null)) return null;
  return { normal: rp.normal ?? null, foil: rp.foil ?? null };
}

export default function CardDetails({ id, details = null, toggleSelected, showCollectionSelect = false }) {
  const ops = useOperations();
  const { collectionsEnabled } = useMode();
  const currentCollection = useCollection();
  const collections = useCollections();
  const cardsDispatch = useCardsDispatch();
  const triggerRefresh = useRefreshCardList();
  const [selectedCollection, setSelectedCollection] = useState(null);
  const prices = usePrices();
  const price = preferredPrices(prices[id]);
  const [purchasePriceInput, setPurchasePriceInput] = useState("");
  const [foilPurchasePriceInput, setFoilPurchasePriceInput] = useState("");

  React.useEffect(() => {
    if (price?.normal != null && purchasePriceInput === "") {
      setPurchasePriceInput(price.normal.toFixed(2));
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [price?.normal]);

  React.useEffect(() => {
    if (price?.foil != null && foilPurchasePriceInput === "") {
      setFoilPurchasePriceInput(price.foil.toFixed(2));
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [price?.foil]);

  const updateQuantity = (delta, deltaFoil, priceInputValue) => {
    let collection = details != null
      ? details.collectionId
      : (showCollectionSelect ? (selectedCollection ?? collections[0]?.id ?? currentCollection) : currentCollection);
    let add = parseInt(delta) >= 0 && parseInt(deltaFoil) >= 0;
    let url =
      "/collection/cards/" + collection + "/" + (add ? "add" : "delete");
    const parsedPrice = parseFloat(priceInputValue);
    let body = {
      id: id,
      collectionId: collection,
      quantity: Math.abs(parseInt(delta)),
      foilQuantity: Math.abs(parseInt(deltaFoil)),
      ...(add && !isNaN(parsedPrice) && parsedPrice > 0 ? { purchasePrice: parsedPrice } : {}),
    };

    ops
      .fetch("Updating quantities for card " + id, {}, url, {
        method: "post",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify(body),
      })
      .then((data) => {
        if (details != null) {
          cardsDispatch({ type: "added", card: add ? data[0] : data });
          triggerRefresh(true);
        } else {
          window.dispatchEvent(new CustomEvent("gathers:collection-updated"));
        }
      });
  };

  return (
    <div className="card-img-overlay d-flex" onClick={toggleSelected}>
      {price && (
        <div className="d-flex flex-column align-items-end gap-1" style={{ position: "absolute", top: 6, right: 6 }}>
          {price.normal != null && (
            <span className="badge bg-success">${price.normal.toFixed(2)}</span>
          )}
          {price.foil != null && (
            <span className="badge" style={{ backgroundColor: "#7c3aed" }}>${price.foil.toFixed(2)} ✦</span>
          )}
        </div>
      )}
      <div className="align-self-center d-flex flex-column gap-1">
        {details != null ? (
          <>
            <div className="d-flex align-items-center gap-1" style={ROW_STYLE}>
              <PriceInput value={purchasePriceInput} onChange={setPurchasePriceInput} />
              <button onClick={() => updateQuantity(1, 0, purchasePriceInput)} className="btn btn-sm btn-outline-success">+</button>
              <span className="badge bg-secondary">{details.quantity}</span>
              <button onClick={() => updateQuantity(-1, 0, "")} className="btn btn-sm btn-outline-danger">-</button>
            </div>
            <div className="d-flex align-items-center gap-1" style={ROW_STYLE}>
              <PriceInput value={foilPurchasePriceInput} onChange={setFoilPurchasePriceInput} />
              <button onClick={() => updateQuantity(0, 1, foilPurchasePriceInput)} className="btn btn-sm btn-outline-success">+</button>
              <span className="badge bg-info">{details.foilQuantity}</span>
              <button onClick={() => updateQuantity(0, -1, "")} className="btn btn-sm btn-outline-danger">-</button>
            </div>
          </>
        ) : collectionsEnabled ? (
          <>
            {showCollectionSelect && collections.length > 0 && (
              <select
                value={selectedCollection ?? collections[0]?.id ?? ""}
                onChange={(e) => setSelectedCollection(e.target.value)}
                onClick={(e) => e.stopPropagation()}
                className="form-select form-select-sm"
              >
                {collections.map((c) => (
                  <option key={c.id} value={c.id}>{c.id}</option>
                ))}
              </select>
            )}
            <div className="d-flex align-items-center gap-1" style={ROW_STYLE}>
              <PriceInput value={purchasePriceInput} onChange={setPurchasePriceInput} />
              <button onClick={() => updateQuantity(1, 0, purchasePriceInput)} className="btn btn-sm btn-light">Add</button>
            </div>
            <div className="d-flex align-items-center gap-1" style={ROW_STYLE}>
              <PriceInput value={foilPurchasePriceInput} onChange={setFoilPurchasePriceInput} />
              <button onClick={() => updateQuantity(0, 1, foilPurchasePriceInput)} className="btn btn-sm btn-info">Add Foil</button>
            </div>
          </>
        ) : null}
      </div>
    </div>
  );
}
