import React from "react";
import { useParams } from "react-router-dom";
import CardDetailLayout from "./CardDetailLayout";

const onZoomMove = (e) => {
  const r = e.currentTarget.getBoundingClientRect();
  e.currentTarget.style.transformOrigin = `${((e.clientX - r.left) / r.width) * 100}% ${((e.clientY - r.top) / r.height) * 100}%`;
};

function renderImage(card) {
  return card.image ? (
    <img src={card.image} alt={card.name} className="img-fluid rounded zoomable" onMouseMove={onZoomMove} />
  ) : (
    <div className="p-3 bg-secondary text-white rounded">No image available</div>
  );
}

function renderRows(card) {
  const artists = Array.isArray(card.artists) ? card.artists.join(", ") : (card.artist ?? "—");
  const domains = Array.isArray(card.domains) ? card.domains.join(", ") : "—";
  return (
    <>
      <tr><th>Set</th><td>{card.setCode}</td></tr>
      <tr><th>Collector Number</th><td>{card.collectorNumber}</td></tr>
      <tr><th>Rarity</th><td>{card.rarity}</td></tr>
      <tr><th>Artist(s)</th><td>{artists}</td></tr>
      <tr><th>Domains</th><td>{domains}</td></tr>
      {card.text && <tr><th>Text</th><td style={{ whiteSpace: "pre-line" }}>{card.text}</td></tr>}
    </>
  );
}

export default function RiftboundCardDetailView() {
  const { id } = useParams();
  return (
    <CardDetailLayout
      fetchUrl={`/api/riftbound/cards?ids=${encodeURIComponent(id)}`}
      cardId={id}
      renderImage={renderImage}
      renderRows={renderRows}
    />
  );
}
