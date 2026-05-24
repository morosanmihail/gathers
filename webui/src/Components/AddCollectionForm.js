import React, { useState } from "react";
import { Button, Form } from "react-bootstrap";
import { useOperations } from "../OperationsContext";
import { useCollectionsDispatch } from "./CollectionContext";

function AddCollectionForm() {
  const [showForm, setShowForm] = useState(false);
  const [newItem, setNewItem] = useState("");
  const [error, setError] = useState(null);

  const collectionsDispatch = useCollectionsDispatch();
  const ops = useOperations();

  const handleToggleForm = () => {
    setShowForm(!showForm);
    setError(null);
  };
  const handleHideForm = () => {
    setShowForm(false);
    setError(null);
  };

  const handleSubmit = (event) => {
    event.preventDefault();
    setError(null);
    ops
      .fetch("Adding new collection", {}, "/api/collection/add", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id: newItem }),
      })
      .then(() => {
        collectionsDispatch({
          type: "added",
          item: { id: newItem },
        });
        setNewItem("");
        setShowForm(false);
      })
      .catch((e) => {
        setError(e.message);
      });
  };

  return (
    <>
      <Button onClick={handleToggleForm}>Add Collection</Button>
      {showForm && (
        <Form onSubmit={handleSubmit}>
          <Form.Group controlId="newItem">
            <Form.Control
              type="text"
              value={newItem}
              placeholder="New Collection"
              onChange={(event) => setNewItem(event.target.value)}
              isInvalid={!!error}
            />
            <Form.Control.Feedback type="invalid">{error}</Form.Control.Feedback>
          </Form.Group>
          <Button variant="primary" type="submit">
            Submit
          </Button>
          <Button variant="secondary" onClick={handleHideForm}>
            Cancel
          </Button>
        </Form>
      )}
    </>
  );
}

export default AddCollectionForm;
