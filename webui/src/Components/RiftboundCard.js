import React from "react";
import CardShell from "./CardShell";

export default function RiftboundCard({ id, card = null, details = null, provider = null, showCollectionSelect = false, listMode = false }) {
  return (
    <CardShell
      id={id}
      card={card}
      details={details}
      provider={provider}
      listMode={listMode}
      showCollectionSelect={showCollectionSelect}
      detailPath={`/card/riftbound/${encodeURIComponent(id)}`}
      getImagePath={(_card) => _card.image ?? ""}
    />
  );
}
