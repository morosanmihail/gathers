-- Generalizes the fixed MTG-shaped (quantity, foilquantity) pair into a
-- generic `finish` dimension, so a card row tracks one specific
-- printing/finish (MTG's nonfoil/foil/etched, Pokemon's holo/reverse-holo,
-- or just one implicit finish for a game like Riftbound with a single
-- version per collector number) instead of always exactly two hardcoded
-- finishes. `''` is the default/primary finish (what `quantity` used to
-- mean); every other finish, including `'foil'`, is now just another row
-- sharing the same `uuid` + `collection`.
--
-- Existing rows split into up to two rows each: a `''` row (from the old
-- `quantity` column, which also keeps `want_quantity` — wanting a card
-- doesn't pin down a finish yet) and a `'foil'` row (from the old
-- `foilquantity` column), so no existing collection data is lost.
CREATE TABLE cards_new (
    uuid TEXT NOT NULL,
    finish TEXT NOT NULL DEFAULT '',
    collection TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    want_quantity INTEGER NOT NULL DEFAULT 0,
    timeadded TEXT NULL,
    timeupdated TEXT NULL,
    provider TEXT NOT NULL,
    PRIMARY KEY (uuid, finish, collection)
);

INSERT INTO cards_new (uuid, finish, collection, quantity, want_quantity, timeadded, timeupdated, provider)
SELECT uuid, '', collection, quantity, want_quantity, timeadded, timeupdated, provider
FROM cards WHERE quantity > 0 OR want_quantity > 0;

INSERT INTO cards_new (uuid, finish, collection, quantity, want_quantity, timeadded, timeupdated, provider)
SELECT uuid, 'foil', collection, foilquantity, 0, timeadded, timeupdated, provider
FROM cards WHERE foilquantity > 0;

DROP TABLE cards;
ALTER TABLE cards_new RENAME TO cards;

CREATE UNIQUE INDEX idx_cards_uuid_finish_collection ON cards(uuid, finish, collection);
CREATE INDEX idx_cards_collection_timeadded ON cards(collection, timeadded);

-- purchase_history mirrors the same change: (quantity, foil_quantity,
-- normal_price_per_unit, foil_price_per_unit) becomes (finish, quantity,
-- price_per_unit), one row per recorded purchase of one card+finish.
CREATE TABLE purchase_history_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_id TEXT NOT NULL,
    card_uuid TEXT NOT NULL,
    finish TEXT NOT NULL DEFAULT '',
    quantity INTEGER NOT NULL,
    price_per_unit REAL,
    provider TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);

INSERT INTO purchase_history_new (collection_id, card_uuid, finish, quantity, price_per_unit, provider, recorded_at)
SELECT collection_id, card_uuid, '', quantity, normal_price_per_unit, provider, recorded_at
FROM purchase_history WHERE quantity > 0;

INSERT INTO purchase_history_new (collection_id, card_uuid, finish, quantity, price_per_unit, provider, recorded_at)
SELECT collection_id, card_uuid, 'foil', foil_quantity, foil_price_per_unit, provider, recorded_at
FROM purchase_history WHERE foil_quantity > 0;

DROP TABLE purchase_history;
ALTER TABLE purchase_history_new RENAME TO purchase_history;

CREATE INDEX idx_purchase_history_card ON purchase_history(collection_id, card_uuid);
CREATE INDEX idx_purchase_history_card_date ON purchase_history(collection_id, card_uuid, recorded_at DESC);
CREATE INDEX idx_purchase_history_collection_date ON purchase_history(collection_id, recorded_at DESC);
