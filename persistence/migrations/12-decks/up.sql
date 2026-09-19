-- Decks: a collection tagged kind = 'deck', whose card list lives in a separate
-- table rather than in cards. The cards table is "copies I own"; a deck row is
-- an allocation of a card to a list, so adding a card to a deck doesn't remove
-- it from any collection.

-- CHECK rather than trusting writers: every read parses this column, and a
-- value no variant matches would disrupt the whole collection list rather than
-- the one row
ALTER TABLE collection ADD COLUMN kind TEXT NOT NULL DEFAULT 'collection'
  CHECK (kind IN ('collection', 'deck'));

-- board is the deck zone a row belongs to, open-ended per game rather than a
-- fixed main/side pair: Riftbound has a legend, runes, and battlefields as
-- well, and Pokemon has one 60-card deck. '' is the default zone. Same
-- reasoning as cards.finish.
--
-- finish is carried so a deck can record that it holds a foil copy
-- specifically. There is no want_quantity: a deck is a list to be played, not
-- a wishlist.
CREATE TABLE deck_cards (
  deck        TEXT NOT NULL REFERENCES collection(name) ON DELETE CASCADE ON UPDATE CASCADE,
  uuid        TEXT NOT NULL,
  finish      TEXT NOT NULL DEFAULT '',
  board       TEXT NOT NULL DEFAULT '',
  quantity    INTEGER NOT NULL,
  provider    TEXT NOT NULL,
  timeadded   TEXT NULL,
  timeupdated TEXT NULL,
  PRIMARY KEY (deck, uuid, finish, board)
);

-- Lists one deck, optionally one board
CREATE INDEX idx_deck_cards_deck_board ON deck_cards(deck, board);
