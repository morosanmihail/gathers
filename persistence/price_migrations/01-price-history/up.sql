-- Daily snapshots of market prices for cards tracked in collections. Lives
-- in its own database (storage.prices.db by default) so it can be enabled,
-- dropped or lost without touching collections. One row per card, retailer
-- and finish per UTC day: a later snapshot the same day replaces it.
CREATE TABLE price_history (
    provider TEXT NOT NULL,
    card_uuid TEXT NOT NULL,
    retailer TEXT NOT NULL,
    -- Same convention as `cards.finish`: '' is the default finish, anything
    -- else ('foil', 'etched', 'reverse holo', ...) is game-specific.
    finish TEXT NOT NULL,
    price REAL NOT NULL,
    currency TEXT NOT NULL,
    -- UTC day, YYYY-MM-DD (a `chrono::NaiveDate`). SQLite's date() only
    -- returns its input unchanged for a valid day in exactly that form (it
    -- gives NULL for other text, hence IS rather than =).
    recorded_on TEXT NOT NULL CHECK (date(recorded_on) IS recorded_on),
    PRIMARY KEY (provider, card_uuid, retailer, finish, recorded_on)
) WITHOUT ROWID;
