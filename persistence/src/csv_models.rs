use serde::{Deserialize, Serialize};

/// Canonical fields making up gathers' collection CSV import/export
/// contract — stable identifiers for what a piece of data *is*, independent
/// of what column header text a particular CSV file actually uses for it
/// (see `CsvLayout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CsvField {
    SetCode,
    CollectorNumber,
    Quantity,
    FoilQuantity,
    Provider,
    /// Which finish `Quantity` and `WantQuantity` are of, using
    /// `models::CollectionCard::finish`'s names. Empty is the default finish,
    /// whose row also carries `FoilQuantity` when that column is exported —
    /// the same shape as exports from before this column existed.
    Finish,
    WantQuantity,
    /// The card's name. Written on export for people reading the file;
    /// ignored on import, where cards are found by `Id` or set and number.
    Name,
    /// The card's id in its provider (`models::CollectionCard::uuid`).
    /// Identifies a card exactly, where set and number can match several
    /// printings.
    Id,
}

/// What import does with a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CsvImportUse {
    /// Finds the card: `Id`, or `SetCode` and `CollectorNumber` together.
    Identifies,
    /// A count; at least one must be mapped.
    Count,
    /// Used when mapped, defaulted when not.
    Optional,
    /// Not read on import.
    Ignored,
}

impl CsvField {
    pub const ALL: [CsvField; 9] = [
        CsvField::SetCode,
        CsvField::CollectorNumber,
        CsvField::Quantity,
        CsvField::FoilQuantity,
        CsvField::Provider,
        CsvField::Finish,
        CsvField::WantQuantity,
        CsvField::Name,
        CsvField::Id,
    ];

    /// The column header gathers' own format uses for this field.
    pub fn default_header(self) -> &'static str {
        match self {
            CsvField::SetCode => "Set",
            CsvField::CollectorNumber => "CollectorNumber",
            CsvField::Quantity => "Quantity",
            CsvField::FoilQuantity => "FoilQuantity",
            CsvField::Provider => "Provider",
            CsvField::Finish => "Finish",
            CsvField::WantQuantity => "WantQuantity",
            CsvField::Name => "Name",
            CsvField::Id => "Id",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CsvField::SetCode => "Set",
            CsvField::CollectorNumber => "Collector number",
            CsvField::Quantity => "Quantity",
            CsvField::FoilQuantity => "Foil quantity",
            CsvField::Provider => "Provider",
            CsvField::Finish => "Finish",
            CsvField::WantQuantity => "Wanted quantity",
            CsvField::Name => "Card name",
            CsvField::Id => "Card id",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            CsvField::SetCode => "Set code (MTG), or set name (Pokémon).",
            CsvField::CollectorNumber => "The card's number within its set.",
            CsvField::Quantity => "Copies owned of the row's finish.",
            CsvField::FoilQuantity => {
                "Foil copies owned. When exported, foil copies share the default finish's row; otherwise they get a row with finish \"foil\"."
            }
            CsvField::Provider => "Which game database the card is from, e.g. MagicSQLite. Empty means MTG.",
            CsvField::Finish => {
                "The row's finish, e.g. foil, etched, Reverse Holofoil. Empty is the default finish. On import Normal/Foil/Yes/No are understood too."
            }
            CsvField::WantQuantity => "Copies wanted (wishlist).",
            CsvField::Name => "The card's name. Export only.",
            CsvField::Id => "The card's id in its game database. Matches a card exactly; on import it is tried before set and number.",
        }
    }

    pub fn import_use(self) -> CsvImportUse {
        match self {
            CsvField::SetCode | CsvField::CollectorNumber | CsvField::Id => CsvImportUse::Identifies,
            CsvField::Quantity | CsvField::FoilQuantity | CsvField::WantQuantity => CsvImportUse::Count,
            CsvField::Provider | CsvField::Finish => CsvImportUse::Optional,
            CsvField::Name => CsvImportUse::Ignored,
        }
    }
}

/// One CSV column: which field it holds and its header text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CsvColumn {
    pub field: CsvField,
    pub header: String,
}

/// Which columns a CSV file has, in order. Export writes exactly these
/// columns; import reads each field from the column with that header (any
/// other columns in the file are ignored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CsvLayout {
    pub columns: Vec<CsvColumn>,
}

/// A built-in layout matching a popular collection manager's CSV format.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct CsvPreset {
    /// Short name it's selected by, e.g. `tcgplayer`.
    pub id: String,
    pub label: String,
    /// What carries over and what doesn't.
    pub description: String,
    pub layout: CsvLayout,
}

impl Default for CsvLayout {
    fn default() -> Self {
        Self::gathers()
    }
}

impl CsvLayout {
    pub fn new(columns: impl IntoIterator<Item = (CsvField, &'static str)>) -> Self {
        Self {
            columns: columns
                .into_iter()
                .map(|(field, header)| CsvColumn { field, header: header.to_string() })
                .collect(),
        }
    }

    /// gathers' own format: everything needed to recreate a collection
    /// exactly. Its first five columns are the format's original ones, so
    /// files from before the rest existed still import, and older versions
    /// can read these (ignoring the columns they don't know).
    pub fn gathers() -> Self {
        Self::new(
            [
                CsvField::SetCode,
                CsvField::CollectorNumber,
                CsvField::Quantity,
                CsvField::FoilQuantity,
                CsvField::Provider,
                CsvField::Finish,
                CsvField::WantQuantity,
                CsvField::Id,
            ]
            .map(|f| (f, f.default_header())),
        )
    }

    /// Every built-in layout, gathers' own first.
    pub fn presets() -> Vec<CsvPreset> {
        let preset = |id: &str, label: &str, description: &str, layout: CsvLayout| CsvPreset {
            id: id.to_string(),
            label: label.to_string(),
            description: description.to_string(),
            layout,
        };
        vec![
            preset(
                "gathers",
                "gathers",
                "gathers' own format. Keeps every finish, wishlist count and exact card.",
                Self::gathers(),
            ),
            preset(
                "tcgplayer",
                "TCGplayer",
                "TCGplayer's collection export. Foil is a Printing per row rather than a separate count.",
                Self::new([
                    (CsvField::Quantity, "Quantity"),
                    (CsvField::Name, "Name"),
                    (CsvField::SetCode, "Set Code"),
                    (CsvField::CollectorNumber, "Card Number"),
                    (CsvField::Finish, "Printing"),
                ]),
            ),
            preset(
                "deckbox",
                "Deckbox",
                "Deckbox's collection export. Its Edition is a full set name, so set codes only match after editing.",
                Self::new([
                    (CsvField::Quantity, "Count"),
                    (CsvField::Name, "Name"),
                    (CsvField::SetCode, "Edition"),
                    (CsvField::CollectorNumber, "Card Number"),
                    (CsvField::Finish, "Foil"),
                ]),
            ),
            preset(
                "cardkingdom",
                "Card Kingdom",
                "Card Kingdom's list format. It has no collector numbers, so it's for exporting only.",
                Self::new([
                    (CsvField::Name, "Card Name"),
                    (CsvField::SetCode, "Edition"),
                    (CsvField::Finish, "Foil"),
                    (CsvField::Quantity, "Quantity"),
                ]),
            ),
            preset(
                "mtggoldfish",
                "MTGGoldfish",
                "MTGGoldfish's collection format. It has no collector numbers, so it's for exporting only.",
                Self::new([
                    (CsvField::Name, "Card"),
                    (CsvField::SetCode, "Set ID"),
                    (CsvField::Quantity, "Quantity"),
                    (CsvField::Finish, "Foil"),
                ]),
            ),
        ]
    }

    /// A built-in layout by id, matched case-insensitively and ignoring
    /// `_`, `-` and spaces (`card-kingdom`, `CardKingdom`), with `goldfish`
    /// also naming MTGGoldfish.
    pub fn preset(name: &str) -> Option<Self> {
        let name = match name.to_lowercase().replace(['_', '-', ' '], "").as_str() {
            "goldfish" => "mtggoldfish".to_string(),
            other => other.to_string(),
        };
        Self::presets().into_iter().find(|p| p.id == name).map(|p| p.layout)
    }

    pub fn column(&self, field: CsvField) -> Option<&CsvColumn> {
        self.columns.iter().find(|c| c.field == field)
    }

    pub fn has(&self, field: CsvField) -> bool {
        self.column(field).is_some()
    }

    /// Uses `header` for `field`'s column, adding the column at the end if
    /// the layout doesn't have it.
    pub fn set_header(&mut self, field: CsvField, header: String) {
        match self.columns.iter_mut().find(|c| c.field == field) {
            Some(column) => column.header = header,
            None => self.columns.push(CsvColumn { field, header }),
        }
    }

    /// Checks the layout can be written: at least one column, headers
    /// non-empty and distinct, each field at most once.
    pub fn validate(&self) -> Result<(), String> {
        if self.columns.is_empty() {
            return Err("Choose at least one column".to_string());
        }
        for (i, column) in self.columns.iter().enumerate() {
            if column.header.trim().is_empty() {
                return Err(format!("The {} column needs a header", column.field.label()));
            }
            for other in &self.columns[..i] {
                if other.header == column.header {
                    return Err(format!("Two columns are both called '{}'", column.header));
                }
                if other.field == column.field {
                    return Err(format!("{} is used for more than one column", column.field.label()));
                }
            }
        }
        Ok(())
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
    /// See `CsvField::Finish`, already normalized (see `normalize_finish`).
    pub finish: String,
    pub want_quantity: u32,
    pub id: String,
}

/// Maps the ways other formats spell the two common finishes onto gathers'
/// names (`""` and `foil`); anything else is kept as given.
pub fn normalize_finish(finish: &str) -> String {
    let finish = finish.trim();
    match finish.to_lowercase().as_str() {
        "" | "normal" | "nonfoil" | "non-foil" | "regular" | "no" | "false" | "0" => String::new(),
        "foil" | "yes" | "true" | "1" => "foil".to_string(),
        _ => finish.to_string(),
    }
}
