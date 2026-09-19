-- `08-card-finishes` recreated `cards` with PRIMARY KEY (uuid, finish, collection)
-- and *also* declared a UNIQUE INDEX over exactly those three columns, in the
-- same order. SQLite already maintains an implicit index for a rowid table's
-- PRIMARY KEY, so the explicit one is redundant
DROP INDEX IF EXISTS idx_cards_uuid_finish_collection;
