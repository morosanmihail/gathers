-- Shareable read-only collection links. A row here is the only way to reach
-- a collection through the public /api/share endpoint: the owner must
-- explicitly create it, and deleting it (revoke) immediately invalidates
-- the link. There is no way to derive a valid token from the collection id.
CREATE TABLE share_links (
    token TEXT PRIMARY KEY,
    collection_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_share_links_collection ON share_links(collection_id);
