-- cards: all WHERE collection=? queries scan the full table because collection
-- is the second column in the (uuid, collection) PK. This index enables seeks.
-- Including timeadded eliminates the temp B-tree sort for the default page order.
CREATE INDEX IF NOT EXISTS idx_cards_collection_timeadded ON cards(collection, timeadded);

-- purchase_history: get_history filters on (collection_id, card_uuid) and orders
-- by recorded_at DESC. Adding recorded_at to the index eliminates the temp sort.
CREATE INDEX IF NOT EXISTS idx_purchase_history_card_date
    ON purchase_history(collection_id, card_uuid, recorded_at DESC);

-- purchase_history: get_all_history filters on collection_id only and also orders
-- by recorded_at DESC. Separate index so the sort can be served without a temp tree.
CREATE INDEX IF NOT EXISTS idx_purchase_history_collection_date
    ON purchase_history(collection_id, recorded_at DESC);
