import React, { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import ViewProviders from "./ViewProviders";

function PricesTable({ prices }) {
  if (!prices?.paper || Object.keys(prices.paper).length === 0) return null;
  return (
    <div className="mt-4">
      <h5>Prices</h5>
      <table className="table table-sm table-bordered">
        <thead>
          <tr>
            <th>Retailer</th>
            <th>Normal</th>
            <th>Foil</th>
          </tr>
        </thead>
        <tbody>
          {Object.entries(prices.paper).map(([retailer, rp]) => (
            <tr key={retailer}>
              <td>{retailer}</td>
              <td>{rp.normal != null ? `$${rp.normal.toFixed(2)}` : "—"}</td>
              <td>{rp.foil != null ? `$${rp.foil.toFixed(2)}` : "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function CardDetailContent({ fetchUrl, cardId, renderImage, renderRows }) {
  const [card, setCard] = useState(null);
  const [prices, setPrices] = useState(null);
  const [error, setError] = useState(null);
  const navigate = useNavigate();

  useEffect(() => {
    fetch(fetchUrl)
      .then((res) => res.json())
      .then((data) => {
        const found = data[cardId];
        if (found) setCard(found);
        else setError("Card not found.");
      })
      .catch(() => setError("Failed to load card."));

    fetch(`/api/mtg/prices?ids=${encodeURIComponent(cardId)}`)
      .then((res) => (res.ok ? res.json() : null))
      .then((data) => { if (data?.[cardId]) setPrices(data[cardId]); })
      .catch(() => {});
  }, [fetchUrl, cardId]);

  if (error) return <p className="p-3 text-danger">{error}</p>;
  if (!card) return <p className="p-3">Loading...</p>;

  return (
    <div className="container mt-4">
      <button className="btn btn-link p-0" onClick={() => navigate(-1)}>
        ← Back
      </button>
      <div className="row mt-3 g-4">
        <div className="col-md-4">{renderImage(card)}</div>
        <div className="col-md-8">
          <h2>{card.name}</h2>
          <table className="table table-sm table-bordered">
            <tbody>{renderRows(card)}</tbody>
          </table>
          <PricesTable prices={prices} />
        </div>
      </div>
    </div>
  );
}

export default function CardDetailLayout({ fetchUrl, cardId, renderImage, renderRows }) {
  return (
    <ViewProviders>
      <CardDetailContent
        fetchUrl={fetchUrl}
        cardId={cardId}
        renderImage={renderImage}
        renderRows={renderRows}
      />
    </ViewProviders>
  );
}
