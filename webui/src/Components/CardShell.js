import React, { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import CardDetails from "./CardDetails";
import { useSelectedCardsDispatch } from "./CardListContexts/SelectedCardsContext";
import { useCardLoader } from "./CardListContexts/CardLoaderContext";

function ProviderIcon({ provider }) {
  if (provider?.includes("Riftbound")) return <span className="card-list-provider-icon" title="Riftbound">⚡</span>;
  if (provider?.includes("Pokemon")) return <span className="card-list-provider-icon" title="Pokémon">🔴</span>;
  return <span className="card-list-provider-icon" title="Magic: The Gathering">🎴</span>;
}

function getArtist(card) {
  if (Array.isArray(card.artists)) return card.artists.join(", ");
  return card.artist ?? "";
}

export default function CardShell({ id, card = null, details = null, provider = null, detailPath, getImagePath, showCollectionSelect = false, listMode = false }) {
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
            {details != null && (
              <>
                <span className="card-list-qty badge bg-secondary">×{details.quantity}</span>
                {details.foilQuantity > 0 && (
                  <span className="card-list-foil badge bg-info text-dark">✦×{details.foilQuantity}</span>
                )}
              </>
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
            </div>
          </div>
        </div>
      )}
    </>
  );
}
