import React, { useState, useEffect } from "react";
import { useCollection } from "../CollectionContext";

export default function CollectionTotalPrice() {
  const collection = useCollection();
  const [data, setData] = useState(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    const handler = () => setTick((n) => n + 1);
    window.addEventListener("gathers:collection-updated", handler);
    return () => window.removeEventListener("gathers:collection-updated", handler);
  }, []);

  useEffect(() => {
    if (!collection) return;
    fetch(`/collection/cards/${encodeURIComponent(collection)}/total_price`)
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => setData(d))
      .catch(() => setData(null));
  }, [collection, tick]);

  if (!data || data.priced_count === 0) return null;

  return (
    <span className="small text-muted" title={`${data.priced_count} of ${data.total_count} cards priced`}>
      Total: <strong className="text-success">${data.total.toFixed(2)}</strong>
      <span className="ms-1 opacity-50">({data.priced_count}/{data.total_count})</span>
    </span>
  );
}
