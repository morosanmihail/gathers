import React from "react";
import { useParams } from "react-router-dom";
import CardDetailLayout from "./CardDetailLayout";

const colorMap = { White: "W", Blue: "U", Black: "B", Red: "R", Green: "G", Colourless: "C", Multicoloured: "M" };

const onZoomMove = (e) => {
  const r = e.currentTarget.getBoundingClientRect();
  e.currentTarget.style.transformOrigin = `${((e.clientX - r.left) / r.width) * 100}% ${((e.clientY - r.top) / r.height) * 100}%`;
};

function renderImage(card) {
  const imagePath = card.cardIdentifiers?.scryfallId
    ? `https://api.scryfall.com/cards/${card.cardIdentifiers.scryfallId}?format=image`
    : null;
  return imagePath ? (
    <img src={imagePath} alt={card.name} className="img-fluid rounded zoomable" onMouseMove={onZoomMove} />
  ) : (
    <div className="p-3 bg-secondary text-white rounded">No image available</div>
  );
}

function renderRows(card) {
  const colorStr = card.colorIdentity?.length
    ? card.colorIdentity.map((c) => colorMap[c] ?? c).join("")
    : "—";
  const typeStr = [
    ...(card.supertypes ?? []),
    ...(card.types ?? []),
    ...(card.subtypes?.length ? ["—", ...card.subtypes] : []),
  ].join(" ");

  return (
    <>
      <tr><th>Set</th><td>{card.setCode}</td></tr>
      <tr><th>Collector Number</th><td>{card.collectorNumber}</td></tr>
      <tr><th>Rarity</th><td>{card.rarity}</td></tr>
      <tr><th>Artist</th><td>{card.artist}</td></tr>
      {typeStr && <tr><th>Type</th><td>{typeStr}</td></tr>}
      <tr><th>Color Identity</th><td>{colorStr}</td></tr>
      {card.text && <tr><th>Text</th><td style={{ whiteSpace: "pre-line" }}>{card.text}</td></tr>}
    </>
  );
}

export default function MtgCardDetailView() {
  const { id } = useParams();
  return (
    <CardDetailLayout
      fetchUrl={`/api/mtg/cards?ids=${encodeURIComponent(id)}`}
      cardId={id}
      renderImage={renderImage}
      renderRows={renderRows}
    />
  );
}
