-- Whether a deck is currently built, and where it physically is. A built
-- deck is assembled in real life, so its cards are committed and
-- unavailable to another deck; an unbuilt deck is only a list and commits
-- nothing. Availability is read off this: all four Spellstutter Sprite are
-- in Dimir Faeries, which is built and in the red deck box.
--
-- Rows are created lazily, so a deck without one is simply not built.
CREATE TABLE deck_meta (
  deck     TEXT PRIMARY KEY REFERENCES collection(name) ON DELETE CASCADE ON UPDATE CASCADE,
  built    INTEGER NOT NULL DEFAULT 0,
  location TEXT NULL,
  notes    TEXT NULL
);

-- Reverse lookup: which decks hold this card. deck_cards' primary key is
-- prefixed by deck, so it can't serve WHERE uuid = ?, and that is the query
-- availability is built on
CREATE INDEX idx_deck_cards_uuid ON deck_cards(uuid, finish);
