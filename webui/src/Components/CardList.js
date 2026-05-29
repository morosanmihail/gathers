import React, { useState, useEffect } from "react";
import Card from "./Card";
import RiftboundCard from "./RiftboundCard";
import PokemonCard from "./PokemonCard";
import { useNavigate, useLocation, useSearchParams } from "react-router-dom";
import { useCollection, usePageNumber } from "./CollectionContext";
import { useOperations } from "../OperationsContext";
import { useSelectedCardsDispatch } from "./CardListContexts/SelectedCardsContext";
import { useSystemType, useSystems } from "./SystemTypeContext";
import ReactPaginate from "react-paginate";
import {
  useCards,
  useCardsDispatch,
  pageSize,
} from "../Components/CardListContexts/CardsContext";
import {
  useRefresh,
  useRefreshCardList,
} from "./CardListContexts/RefreshCardListContext";
import { useCollectionFilters, collectionFiltersActive } from "./CollectionFilterBar";
import { useFetchPrices } from "./CardListContexts/PricesContext";
import { usePricingEnabled } from "./SystemTypeContext";

const HEADER_COLS = [
  { field: "Name",     label: "Name",    className: "card-list-name",    sortable: true },
  { field: "SetCode",  label: "Set",     className: "card-list-set",     sortable: true },
  { field: "Rarity",   label: "Rarity",  className: "card-list-rarity",  sortable: true },
  { field: "Artist",   label: "Artist",  className: "card-list-artist",  sortable: true },
  { field: null,       label: "Price",    className: "card-list-price",       sortable: false },
  { field: null,       label: "Non-Foil", className: "card-list-qty-actions", sortable: false },
  { field: null,       label: "Foil",     className: "card-list-qty-actions", sortable: false },
  { field: null,       label: "History",  className: "card-list-history",     sortable: false },
];

function ListHeader({ sortBy, sortOrder }) {
  const pricingEnabled = usePricingEnabled();
  const [searchParams, setSearchParams] = useSearchParams();

  const handleSort = (field) => {
    const next = new URLSearchParams(searchParams);
    if (field === sortBy) {
      next.set("cf_sortOrder", sortOrder === "Asc" ? "Desc" : "Asc");
    } else {
      next.set("cf_sortBy", field);
      next.set("cf_sortOrder", "Asc");
    }
    next.set("page", "1");
    setSearchParams(next);
  };

  return (
    <div className="card-list-header">
      <span className="card-list-provider-icon" />
      {HEADER_COLS.filter(({ label }) =>
        pricingEnabled || (label !== "Price" && label !== "History")
      ).map(({ field, label, className, sortable }) => {
        const active = sortable && sortBy === field;
        return (
          <span
            key={label}
            className={`${className}${sortable ? " card-list-header-col" : ""}${active ? " active" : ""}`}
            onClick={sortable ? () => handleSort(field) : undefined}
            title={sortable ? `Sort by ${label}` : undefined}
          >
            {label}
            {active && <span className="card-list-sort-arrow">{sortOrder === "Asc" ? " ↑" : " ↓"}</span>}
          </span>
        );
      })}
    </div>
  );
}

function CardComponent({ viewMode, systemType, id, details }) {
  const effectiveSystem = details?.provider || systemType;
  if (effectiveSystem === "RiftboundSQLite") {
    return <RiftboundCard id={id} details={details} provider={effectiveSystem} listMode={viewMode === "list"} />;
  } else if (effectiveSystem === "PokemonSQLite") {
    return <PokemonCard id={id} details={details} provider={effectiveSystem} listMode={viewMode === "list"} />;
  }
  return <Card id={id} details={details} provider={effectiveSystem} listMode={viewMode === "list"} />;
}

const COLLECTION_SORT_FIELDS = new Set(["TimeAdded", "Quantity", "FoilQuantity", "Provider"]);

function sortIsCardLevel(sortBy) {
  return sortBy && !COLLECTION_SORT_FIELDS.has(sortBy);
}

function buildListUrl(collection, filters, pageNumber, systems) {
  const params = new URLSearchParams();
  params.set("offset", String((pageNumber - 1) * pageSize));
  params.set("limit", String(pageSize));
  if (filters.sortBy && COLLECTION_SORT_FIELDS.has(filters.sortBy)) params.set("sort_by", filters.sortBy);
  if (filters.sortOrder && filters.sortOrder !== "Asc") params.set("sort_order", filters.sortOrder);
  if (filters.provider) {
    params.set("provider", filters.provider);
  } else if (systems.length > 0) {
    params.set("providers", systems.join(","));
  }
  return `/api/collection/cards/${collection}/list?${params.toString()}`;
}

function buildSearchBody(filters) {
  const body = {};
  if (filters.name)    body.name = filters.name;
  if (filters.setCode) body.setCode = filters.setCode;
  if (filters.rarity)  body.rarity = filters.rarity;
  if (filters.artist)  body.artist = filters.artist;
  if (filters.text)    body.text = filters.text;
  if (filters.colorIdentities.length > 0) body.colorIdentities = filters.colorIdentities;
  if (filters.domains.length > 0)         body.domains = filters.domains;
  if (filters.energyTypes.length > 0)     body.energyTypes = filters.energyTypes;
  if (filters.sortBy)    body.sortBy = filters.sortBy;
  if (filters.sortOrder) body.sortOrder = filters.sortOrder;
  return body;
}

function impliedProvider(filters) {
  if (filters.colorIdentities.length > 0) return "MagicSQLite";
  if (filters.domains.length > 0)         return "RiftboundSQLite";
  if (filters.energyTypes.length > 0)     return "PokemonSQLite";
  return null;
}

function buildSearchUrl(collection, filters, pageNumber, systems, isCount = false) {
  const params = new URLSearchParams();
  params.set("offset", String((pageNumber - 1) * pageSize));
  params.set("limit", String(pageSize));
  const provider = filters.provider || impliedProvider(filters);
  if (provider) {
    params.set("provider", provider);
  } else if (systems.length > 0) {
    params.set("providers", systems.join(","));
  }
  const base = `/api/collection/cards/${collection}/search`;
  return isCount ? `${base}/count?${params.toString()}` : `${base}?${params.toString()}`;
}

export default function CardList() {
  const navigate = useNavigate();
  const location = useLocation();
  const ops = useOperations();
  const collection = useCollection();
  const pageNumber = usePageNumber();
  const selectedDispatch = useSelectedCardsDispatch();
  const systemType = useSystemType();
  const systems = useSystems();
  const refresh = useRefresh();
  const setRefresh = useRefreshCardList();
  const filters = useCollectionFilters();
  const filtersActive = collectionFiltersActive(filters) || sortIsCardLevel(filters.sortBy);

  const cards = useCards();
  const cardsDispatch = useCardsDispatch();
  const fetchPrices = useFetchPrices();
  const [loading, setLoading] = useState(true);
  const [cardCount, setCardCount] = useState(0);
  const [localRefresh, setLocalRefresh] = useState(0);

  useEffect(() => {
    const handler = () => setLocalRefresh((n) => n + 1);
    window.addEventListener("gathers:collection-updated", handler);
    return () => window.removeEventListener("gathers:collection-updated", handler);
  }, []);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const filterDeps = [
    filtersActive, filters.name, filters.setCode, filters.rarity, filters.artist,
    filters.text, filters.provider, filters.sortBy, filters.sortOrder,
    JSON.stringify(filters.colorIdentities),
    JSON.stringify(filters.domains),
    JSON.stringify(filters.energyTypes),
  ];

  useEffect(() => {
    setLoading(true);

    if (filtersActive) {
      const body = buildSearchBody(filters);
      const searchUrl = buildSearchUrl(collection, filters, pageNumber, systems);
      const countUrl = buildSearchUrl(collection, filters, pageNumber, systems, true);

      ops
        .fetch("Filtering collection", [], searchUrl, {
          method: "post",
          headers: { Accept: "application/json", "Content-Type": "application/json" },
          body: JSON.stringify(body),
        })
        .then((data) => {
          cardsDispatch({ type: "overwrite", cards: data });
          setLoading(false);
          setRefresh(false);
          selectedDispatch({ type: "empty" });
          fetchPrices({
            mtgIds: data.filter((c) => !c.provider || c.provider === "MagicSQLite").map((c) => c.id),
            pokemonIds: data.filter((c) => c.provider === "PokemonSQLite").map((c) => c.id),
          });
        });

      ops
        .fetch("Getting filtered count", 0, countUrl, {
          method: "post",
          headers: { Accept: "application/json", "Content-Type": "application/json" },
          body: JSON.stringify(body),
        })
        .then((data) => setCardCount(data));
    } else {
      const listUrl = buildListUrl(collection, filters, pageNumber, systems);

      ops
        .fetch("Listing items in " + collection, [], listUrl)
        .then((data) => {
          cardsDispatch({ type: "overwrite", cards: data });
          setLoading(false);
          setRefresh(false);
          selectedDispatch({ type: "empty" });
          fetchPrices({
            mtgIds: data.filter((c) => !c.provider || c.provider === "MagicSQLite").map((c) => c.id),
            pokemonIds: data.filter((c) => c.provider === "PokemonSQLite").map((c) => c.id),
          });
        });

      const countParams = new URLSearchParams();
      if (filters.provider) {
        countParams.set("provider", filters.provider);
      } else if (systems.length > 0) {
        countParams.set("providers", systems.join(","));
      }
      ops
        .fetch("Getting card count in " + collection, 0, `/api/collection/cards/${collection}/count?${countParams.toString()}`)
        .then((data) => setCardCount(data));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [collection, pageNumber, refresh, localRefresh, ...filterDeps]);

  const handlePageChange = (event) => {
    navigate("/c/" + collection + "/" + (parseInt(event.selected) + 1) + location.search);
  };

  const viewMode = filters.viewMode;

  return (
    <>
      <div className={viewMode === "list" ? "card-list" : "card-grid list"}>
        {viewMode === "list" && (
          <ListHeader sortBy={filters.sortBy} sortOrder={filters.sortOrder} />
        )}
        {(loading || refresh) && cards.length === 0 ? (
          <p>Loading...</p>
        ) : (
          <React.Fragment>
            {cards.map((card) => (
              <CardComponent
                viewMode={viewMode}
                systemType={systemType}
                id={card.id}
                details={card}
                key={card.collectionId + "-" + card.id}
              />
            ))}
          </React.Fragment>
        )}
      </div>
      <ReactPaginate
        previousLabel="Previous"
        nextLabel="Next"
        pageClassName="page-item"
        pageLinkClassName="page-link"
        previousClassName="page-item"
        previousLinkClassName="page-link"
        nextClassName="page-item"
        nextLinkClassName="page-link"
        breakLabel="..."
        breakClassName="page-item"
        breakLinkClassName="page-link"
        containerClassName="pagination"
        activeClassName="active"
        pageCount={Math.ceil(parseInt(cardCount) / pageSize)}
        marginPagesDisplayed={2}
        pageRangeDisplayed={5}
        onPageChange={handlePageChange}
        forcePage={cardCount > 0 ? Math.max(0, pageNumber - 1) : -1}
      />
    </>
  );
}
