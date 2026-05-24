CREATE TABLE purchase_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_id TEXT NOT NULL,
    card_uuid TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    foil_quantity INTEGER NOT NULL,
    price_per_unit REAL,
    provider TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);

CREATE INDEX idx_purchase_history_card ON purchase_history(collection_id, card_uuid);
