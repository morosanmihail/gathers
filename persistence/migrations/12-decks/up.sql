-- Decks: a collection tagged kind = 'deck', whose card list lives in a separate
-- table rather than in cards. The cards table is "copies I own"; a deck row is
-- an allocation of a card to a list, so adding a card to a deck doesn't remove
-- it from any collection.

-- CHECK rather than trusting writers: every read parses this column, and a
-- value no variant matches would disrupt the whole collection list rather than
-- the one row
ALTER TABLE collection ADD COLUMN kind TEXT NOT NULL DEFAULT 'collection'
  CHECK (kind IN ('collection', 'deck'));

-- The deck's zones, stored rather than derived from the cards so a board can be
-- empty, carry an order, and be renamed. Free-form text per game: Riftbound has
-- a legend, runes and battlefields, Pokemon has one 60-card deck, and a player
-- may want 'Mainboard:Lands'. A new deck starts with Mainboard and Sideboard.
CREATE TABLE deck_boards (
  deck     TEXT NOT NULL REFERENCES collection(name) ON DELETE CASCADE ON UPDATE CASCADE,
  board    TEXT NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY (deck, board)
);

-- finish is carried so a deck can record that it holds a foil copy
-- specifically.
--
-- quantity and want_quantity mean the same as in cards, and a deck row may
-- carry both at once: four Tarmogoyf wanted, three of them already to hand.
-- They are not added together; see deck_cards::count_cards.
--
-- The board reference cascades, so renaming a board moves its cards with it and
-- deleting one takes them away.
CREATE TABLE deck_cards (
  deck          TEXT NOT NULL REFERENCES collection(name) ON DELETE CASCADE ON UPDATE CASCADE,
  uuid          TEXT NOT NULL,
  finish        TEXT NOT NULL DEFAULT '',
  board         TEXT NOT NULL DEFAULT 'Mainboard',
  quantity      INTEGER NOT NULL,
  want_quantity INTEGER NOT NULL DEFAULT 0,
  provider      TEXT NOT NULL,
  timeadded     TEXT NULL,
  timeupdated   TEXT NULL,
  PRIMARY KEY (deck, uuid, finish, board),
  FOREIGN KEY (deck, board) REFERENCES deck_boards(deck, board) ON DELETE CASCADE ON UPDATE CASCADE
);

-- Lists one deck, optionally one board
CREATE INDEX idx_deck_cards_deck_board ON deck_cards(deck, board);
