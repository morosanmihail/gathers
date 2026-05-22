import React, { useState } from "react";
import { useOperations } from "../../OperationsContext";
import { useCollection } from "../CollectionContext";
import { useRefreshCardList } from "../CardListContexts/RefreshCardListContext";

export default function ImportCards() {
  const ops = useOperations();
  const collection = useCollection();
  const triggerRefresh = useRefreshCardList();

  const [file, setFile] = useState();
  const [error, setError] = useState(null);

  const handleFileChange = (e) => {
    if (e.target.files) {
      setFile(e.target.files[0]);
    }
  };

  const handleUploadClick = () => {
    if (!file) {
      return;
    }

    const formData = new FormData();
    formData.append("file", file);
    formData.append("collection", collection);

    setError(null);
    ops
      .fetch("Importing into " + collection, [], "/collection/import", {
        method: "post",
        body: formData,
      })
      .then(() => triggerRefresh(true))
      .catch((e) => setError(e.message));
  };

  return (
    <form className="d-flex flex-column gap-1">
      <div className="input-group">
        <input
          onChange={handleFileChange}
          type="file"
          className="form-control"
          id="inputGroupFile02"
        />
        <button
          onClick={handleUploadClick}
          className="btn btn-outline-secondary"
          type="button"
          id="inputGroupFileAddon04"
        >
          Import
        </button>
      </div>
      {error && <div className="text-danger small">{error}</div>}
    </form>
  );
}
