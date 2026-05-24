import React from "react";
import { useCollection } from "../CollectionContext";

export default function ExportCollection() {
  const collection = useCollection();

  return (
    <div className="d-flex">
      <a href={"/api/collection/export/" + collection}>
        <button type="button" className="btn btn-info">
          Export
        </button>
      </a>
    </div>
  );
}
