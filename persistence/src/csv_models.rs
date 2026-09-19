use std::collections::HashMap;

/// Canonical fields making up gathers' collection CSV import/export
/// contract — stable identifiers for what a piece of data *is*, independent
/// of what column header text a particular CSV file actually uses for it
/// (see `CsvFieldMapping`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CsvField {
    SetCode,
    CollectorNumber,
    Quantity,
    FoilQuantity,
    Provider,
}

impl CsvField {
    pub const ALL: [CsvField; 5] = [
        CsvField::SetCode,
        CsvField::CollectorNumber,
        CsvField::Quantity,
        CsvField::FoilQuantity,
        CsvField::Provider,
    ];

    /// The column header gathers has always used for this field. Used for
    /// any field not given an explicit override in a `CsvFieldMapping`, so
    /// an empty/default mapping reproduces the original fixed CSV format
    /// exactly — existing exported files and import tooling keep working
    /// unchanged.
    pub fn default_header(self) -> &'static str {
        match self {
            CsvField::SetCode => "Set",
            CsvField::CollectorNumber => "CollectorNumber",
            CsvField::Quantity => "Quantity",
            CsvField::FoilQuantity => "FoilQuantity",
            CsvField::Provider => "Provider",
        }
    }
}

/// Per-field column header overrides for CSV import/export, e.g. "the
/// `CollectorNumber` field is called `Number` in this file". A field left
/// out of the mapping falls back to `CsvField::default_header`, so the
/// default (empty) mapping reproduces gathers' original fixed CSV format
/// exactly — this is what keeps old callers working unchanged.
///
/// The same table is used in both directions: export looks up a field's
/// header to write it under, import inverts it (`header_to_field`) to
/// figure out which column of an arbitrary file holds which field.
#[derive(Debug, Clone, Default)]
pub struct CsvFieldMapping(HashMap<CsvField, String>);

impl CsvFieldMapping {
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the column header used for `field`. A `None` or empty
    /// `header` clears any existing override, reverting to the default.
    pub fn set(&mut self, field: CsvField, header: Option<String>) {
        match header {
            Some(h) if !h.is_empty() => {
                self.0.insert(field, h);
            }
            _ => {
                self.0.remove(&field);
            }
        }
    }

    /// The column header `field` should be written under (export) or
    /// looked for under (import): the override if one was set, otherwise
    /// `CsvField::default_header`.
    pub fn header_for(&self, field: CsvField) -> String {
        self.0
            .get(&field)
            .cloned()
            .unwrap_or_else(|| field.default_header().to_string())
    }

    /// Inverts the mapping into column-header → field, one entry per
    /// `CsvField`, filled in with defaults for anything not overridden.
    /// Used when reading a file to figure out which of its columns (by
    /// header text) holds which canonical field.
    pub fn header_to_field(&self) -> HashMap<String, CsvField> {
        CsvField::ALL.into_iter().map(|f| (self.header_for(f), f)).collect()
    }

    /// A built-in mapping for a popular third-party collection manager's
    /// own CSV column names, keyed by a short name matched
    /// case-insensitively (`tcgplayer`, `cardkingdom`, `deckbox`,
    /// `mtggoldfish`). `None` if `name` isn't one of these.
    ///
    /// Each preset only covers the fields that platform's format actually
    /// has a column for — see the individual constructors below for what's
    /// left at gathers' own default header, and why.
    pub fn preset(name: &str) -> Option<Self> {
        match name.to_lowercase().replace(['_', '-', ' '], "").as_str() {
            "tcgplayer" => Some(Self::tcgplayer()),
            "cardkingdom" => Some(Self::card_kingdom()),
            "deckbox" => Some(Self::deckbox()),
            "mtggoldfish" | "goldfish" => Some(Self::mtggoldfish()),
            _ => None,
        }
    }

    /// TCGplayer's collection export format: `Quantity,Name,Simple Name,
    /// Set,Card Number,Set Code,Printing,Condition,Language,Rarity,Product
    /// ID,SKU`. TCGplayer records foil as a `Printing` value ("Foil" /
    /// "Normal") on the same row as `Quantity`, not as a separate count —
    /// there's no column `FoilQuantity` can map onto, so cards imported
    /// through this preset land as entirely non-foil. `Provider` isn't
    /// part of their format either.
    pub fn tcgplayer() -> Self {
        let mut m = Self::new();
        m.set(CsvField::SetCode, Some("Set Code".to_string()));
        m.set(CsvField::CollectorNumber, Some("Card Number".to_string()));
        m.set(CsvField::Quantity, Some("Quantity".to_string()));
        m
    }

    /// Card Kingdom's buylist CSV format: `Card Name, Edition, Foil,
    /// Quantity`. Cards are identified by name + edition there, with no
    /// collector-number column at all, so importing a genuine Card
    /// Kingdom file will still fail with a clear "missing CollectorNumber
    /// column" error — this preset is mainly useful for *exporting* a
    /// collection labeled with Card Kingdom's own field names. `Edition`
    /// is a full set name ("Magic 2013"), not gathers' short `setCode`
    /// ("M13"), so even the mapped `SetCode` column won't resolve cards on
    /// import without hand-editing.
    pub fn card_kingdom() -> Self {
        let mut m = Self::new();
        m.set(CsvField::SetCode, Some("Edition".to_string()));
        m.set(CsvField::Quantity, Some("Quantity".to_string()));
        m
    }

    /// Deckbox's collection export format: `Count,Tradelist Count,Name,
    /// Edition,Card Number,Condition,Language,Foil`. As with TCGplayer,
    /// `Foil` is a per-row marker rather than a separate count, so
    /// `FoilQuantity` stays unmapped and imported cards land as non-foil.
    /// `Edition` is a full set name — same caveat as Card Kingdom above.
    pub fn deckbox() -> Self {
        let mut m = Self::new();
        m.set(CsvField::SetCode, Some("Edition".to_string()));
        m.set(CsvField::CollectorNumber, Some("Card Number".to_string()));
        m.set(CsvField::Quantity, Some("Count".to_string()));
        m
    }

    /// MTGGoldfish's collection export format: `Card,Set ID,Set Name,
    /// Quantity,Foil,Variation`. Like Card Kingdom, there's no
    /// collector-number column — cards are identified by name + set only —
    /// so importing a real MTGGoldfish file will fail with a clear
    /// missing-column error unless a `CollectorNumber` column is added by
    /// hand first.
    pub fn mtggoldfish() -> Self {
        let mut m = Self::new();
        m.set(CsvField::SetCode, Some("Set ID".to_string()));
        m.set(CsvField::Quantity, Some("Quantity".to_string()));
        m
    }
}

/// One resolved row of collection CSV data, independent of what column
/// headers the source/destination file actually uses for it.
#[derive(Debug, Clone, Default)]
pub struct CSVCard {
    pub set_code: String,
    pub collector_number: String,
    pub quantity: u32,
    pub foil_quantity: u32,
    pub provider: String,
}
