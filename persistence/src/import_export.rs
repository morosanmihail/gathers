use std::collections::{HashMap, HashSet};

use retrieval::{NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait as _};

use crate::{CollectionCard, CollectionCardsParams, PersistenceError, PersistenceSystem, PersistenceSystemTrait as _};
use crate::csv_models::{CSVCard, CsvField, CsvImportUse, CsvLayout, normalize_finish};

fn systems_by_name(retrievals: &[RetrievalSystem]) -> HashMap<&str, &RetrievalSystem> {
    retrievals.iter().map(|r| (r.name(), r)).collect()
}

/// Reads every row of `filename` into `CSVCard`s, taking each field from the
/// column `layout` gives it (columns of the file the layout doesn't mention
/// are ignored). The file needs a way to find each card — an `Id` column, or
/// `SetCode` and `CollectorNumber` columns — and at least one count column.
/// Fields without a column default to an empty string / 0, as do empty cells
/// of count columns.
fn read_csv(filename: &str, layout: &CsvLayout) -> eyre::Result<Vec<CSVCard>> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).trim(csv::Trim::All).from_path(filename)?;
    let headers = rdr.headers()?.clone();

    let mut field_index: HashMap<CsvField, usize> = HashMap::new();
    for column in &layout.columns {
        if column.field.import_use() == CsvImportUse::Ignored {
            continue;
        }
        if let Some(i) = headers.iter().position(|h| h == column.header.trim()) {
            field_index.insert(column.field, i);
        }
    }
    let header_of = |field: CsvField| {
        layout.column(field).map_or_else(|| field.default_header().to_string(), |c| c.header.clone())
    };

    if !field_index.contains_key(&CsvField::Id) {
        for required in [CsvField::SetCode, CsvField::CollectorNumber] {
            if !field_index.contains_key(&required) {
                return Err(eyre::eyre!(
                    "CSV is missing a '{}' column (needed to find cards without an '{}' column)",
                    header_of(required),
                    header_of(CsvField::Id),
                ));
            }
        }
    }
    let counts = [CsvField::Quantity, CsvField::FoilQuantity, CsvField::WantQuantity];
    if !counts.iter().any(|f| field_index.contains_key(f)) {
        return Err(eyre::eyre!(
            "CSV has no quantity column (expected one of {})",
            counts.map(|f| format!("'{}'", header_of(f))).join(", ")
        ));
    }

    let mut cards = vec![];
    for result in rdr.records() {
        let record = result?;
        let get = |field: CsvField| -> &str {
            field_index.get(&field).and_then(|&i| record.get(i)).unwrap_or_default()
        };
        let parse_count = |field: CsvField| -> eyre::Result<u32> {
            let v = get(field);
            if v.is_empty() {
                return Ok(0);
            }
            v.parse::<u32>()
                .ok()
                .filter(|n| *n <= i32::MAX as u32)
                .ok_or_else(|| eyre::eyre!(
                    "invalid value {v:?} in '{}' column (expected a whole number from 0 to {})",
                    header_of(field),
                    i32::MAX
                ))
        };
        cards.push(CSVCard {
            set_code: get(CsvField::SetCode).to_string(),
            collector_number: get(CsvField::CollectorNumber).to_string(),
            quantity: parse_count(CsvField::Quantity)?,
            foil_quantity: parse_count(CsvField::FoilQuantity)?,
            provider: get(CsvField::Provider).to_string(),
            finish: normalize_finish(get(CsvField::Finish)),
            want_quantity: parse_count(CsvField::WantQuantity)?,
            id: get(CsvField::Id).to_string(),
        });
    }
    Ok(cards)
}

/// Folds rows naming the same card and finish (same provider, id, set,
/// collector number and finish) into one, summing their quantities —
/// otherwise only the first such row would be matched back to the resolved
/// card and the rest silently lost. Sums saturate at `i32::MAX`, the most a
/// collection row can hold.
fn merge_duplicate_rows(cards: Vec<CSVCard>) -> Vec<CSVCard> {
    let mut merged: Vec<CSVCard> = Vec::with_capacity(cards.len());
    let mut index: HashMap<(String, String, String, String, String), usize> = HashMap::new();
    for card in cards {
        let key = (
            card.provider.clone(),
            card.id.clone(),
            card.set_code.clone(),
            card.collector_number.clone(),
            card.finish.clone(),
        );
        match index.get(&key) {
            Some(&i) => {
                let max = i32::MAX as u32;
                let existing = &mut merged[i];
                existing.quantity = existing.quantity.saturating_add(card.quantity).min(max);
                existing.foil_quantity = existing.foil_quantity.saturating_add(card.foil_quantity).min(max);
                existing.want_quantity = existing.want_quantity.saturating_add(card.want_quantity).min(max);
            }
            None => {
                index.insert(key, merged.len());
                merged.push(card);
            }
        }
    }
    merged
}

/// A card's name, for the `Name` column.
fn card_name(card: &models::Card) -> &str {
    match card {
        models::Card::Magic(c) => &c.name,
        models::Card::Riftbound(c) => &c.name,
        models::Card::Pokemon(c) => &c.name,
    }
}

impl PersistenceSystem {
    pub async fn import_csv(
        &mut self,
        filename: String,
        collection_name: String,
        retrievals: &[RetrievalSystem],
        progress_sender: Option<tokio::sync::watch::Sender<f32>>,
        layout: &CsvLayout,
    ) -> eyre::Result<()> {
        const DEFAULT_PROVIDER: &str = "MagicSQLite";
        const BULK_CHUNK_SIZE: usize = 500;

        // Everything that can go wrong from here until the collection is
        // written is down to the file's contents, not storage.
        let invalid = |e: eyre::Report| PersistenceError::InvalidInput(format!("{e:#}"));

        let cards = merge_duplicate_rows(read_csv(&filename, layout).map_err(invalid)?);

        let by_name = systems_by_name(retrievals);

        // Group cards by provider. A file without providers (gathers' own
        // original format, and every third-party preset) is MTG, or whatever
        // single system is configured when MTG isn't.
        let mut groups: HashMap<&str, Vec<&CSVCard>> = Default::default();
        for card in &cards {
            groups.entry(card.provider.as_str()).or_default().push(card);
        }

        // (uuid, row, provider_name)
        let mut cta: Vec<(String, &CSVCard, String)> = vec![];
        for (provider, group) in &groups {
            let system = if provider.is_empty() {
                by_name
                    .get(DEFAULT_PROVIDER)
                    .copied()
                    .or_else(|| retrievals.first())
                    .ok_or_else(|| eyre::eyre!("No retrieval system available for import"))?
            } else {
                // Looking an unknown provider's set codes up in some other
                // game would at best find nothing and at worst the wrong cards.
                by_name.get(provider).copied().ok_or_else(|| {
                    let mut known: Vec<&str> = by_name.keys().copied().collect();
                    known.sort_unstable();
                    PersistenceError::InvalidInput(format!(
                        "Unknown provider '{provider}' in CSV (configured: {})",
                        known.join(", ")
                    ))
                })?
            };

            // Rows with an id are matched by it exactly; the rest, and any
            // whose id isn't found, by set and number.
            let mut ids: Vec<String> = group.iter().filter(|c| !c.id.is_empty()).map(|c| c.id.clone()).collect();
            ids.sort_unstable();
            ids.dedup();
            let mut found_ids: HashSet<String> = HashSet::new();
            for chunk in ids.chunks(BULK_CHUNK_SIZE) {
                found_ids.extend(system.get_cards_by_ids(chunk.to_vec()).await?.into_keys());
            }
            let by_set: Vec<&CSVCard> = group
                .iter()
                .copied()
                .filter(|c| !found_ids.contains(&c.id))
                .filter(|c| !c.set_code.is_empty() || !c.collector_number.is_empty())
                .collect();
            for c in group.iter().filter(|c| found_ids.contains(&c.id)) {
                cta.push((c.id.clone(), c, system.name().to_string()));
            }

            // Rows for several finishes of one card share a lookup.
            let mut input: Vec<(String, String)> = by_set
                .iter()
                .map(|c| (c.set_code.clone(), c.collector_number.clone()))
                .collect();
            input.sort_unstable();
            input.dedup();

            let mut resolved = vec![];
            for chunk in input.chunks(BULK_CHUNK_SIZE) {
                resolved.extend(system.bulk_search_cards(chunk.to_vec()).await?);
            }

            for (set_code, collector_number, uuid) in resolved {
                for c in by_set
                    .iter()
                    .filter(|c| c.set_code == set_code && c.collector_number == collector_number)
                {
                    cta.push((uuid.clone(), c, system.name().to_string()));
                }
            }
        }

        if cta.is_empty() {
            return Err(PersistenceError::InvalidInput(
                "No cards could be resolved from the CSV".to_string(),
            )
            .into());
        }

        let now = chrono::Utc::now();
        let time_added = now.to_rfc3339();
        let collection_id = self.add_collection(collection_name).await?;
        let total = cta.len() as f32;
        let mut i: f32 = 0.0;

        for g in cta.chunks(50) {
            // `Quantity`/`WantQuantity` are of the row's own finish, while
            // `FoilQuantity` is always of `foil` — so each resolved row
            // expands into up to two finish rows.
            let mut batch: Vec<CollectionCard> = vec![];
            for (uuid, row, provider) in g {
                let mut push = |finish: &str, quantity: u32, want_quantity: u32| {
                    if quantity > 0 || want_quantity > 0 {
                        batch.push(CollectionCard {
                            uuid: uuid.clone(),
                            finish: finish.to_string(),
                            quantity: quantity as i32,
                            want_quantity: want_quantity as i32,
                            collection: collection_id.clone(),
                            time_added: time_added.clone(),
                            provider: provider.clone(),
                        });
                    }
                };
                push(&row.finish, row.quantity, row.want_quantity);
                push("foil", row.foil_quantity, 0);
            }
            self.add_cards_to_collection(&collection_id, &batch).await?;

            i += g.len() as f32;
            if let Some(ref sender) = progress_sender {
                sender.send(i / total)?;
            }
        }

        Ok(())
    }

    /// Writes the collection as CSV with `layout`'s columns. One row per
    /// card and finish, except that when the layout has a `FoilQuantity`
    /// column a card's `foil` copies go on its default finish's row (as
    /// gathers' original two-count format did). Rows with nothing to show
    /// in the layout's count columns are left out — e.g. wishlist-only
    /// cards when there's no `WantQuantity` column.
    pub async fn export_collection(
        &self,
        collection_id: &models::CollectionID,
        retrievals: &[RetrievalSystem],
        layout: &CsvLayout,
    ) -> eyre::Result<String> {
        layout.validate().map_err(PersistenceError::InvalidInput)?;
        let by_name = systems_by_name(retrievals);

        let mut offset = 0;
        let limit = 500;
        let mut all_cards = vec![];
        loop {
            let page = self
                .get_cards_in_collection_paginated(collection_id, CollectionCardsParams::new(offset, limit))
                .await?;
            if page.is_empty() {
                break;
            }
            offset += page.len();
            all_cards.extend(page);
        }

        #[derive(Default)]
        struct ExportRow {
            quantity: u32,
            foil_quantity: u32,
            want_quantity: u32,
            provider: String,
        }
        let foil_column = layout.has(CsvField::FoilQuantity);
        // Keyed by (uuid, CSV `Finish`).
        let mut rows: HashMap<(String, String), ExportRow> = HashMap::new();
        for card in &all_cards {
            let quantity = card.quantity.max(0) as u32;
            let want_quantity = card.want_quantity.max(0) as u32;
            let merge_foil = foil_column && card.finish == "foil";
            let csv_finish = if merge_foil { "" } else { card.finish.as_str() };
            let entry = rows
                .entry((card.uuid.clone(), csv_finish.to_string()))
                .or_insert_with(|| ExportRow { provider: card.provider.clone(), ..Default::default() });
            if merge_foil {
                entry.foil_quantity += quantity;
            } else {
                entry.quantity += quantity;
            }
            entry.want_quantity += want_quantity;
        }
        rows.retain(|_, row| {
            (layout.has(CsvField::Quantity) && row.quantity > 0)
                || (foil_column && row.foil_quantity > 0)
                || (layout.has(CsvField::WantQuantity) && row.want_quantity > 0)
        });

        // Group UUIDs by stored provider so we issue one lookup per system.
        let mut ids_by_provider: HashMap<&str, Vec<String>> = Default::default();
        for ((uuid, _), row) in &rows {
            let ids = ids_by_provider.entry(row.provider.as_str()).or_default();
            if !ids.contains(uuid) {
                ids.push(uuid.clone());
            }
        }

        let mut looked_up: HashMap<String, (models::Card, String)> = Default::default();
        for (provider, ids) in &ids_by_provider {
            if let Some(system) = by_name.get(provider)
                && let Ok(result) = system.get_cards_by_ids(ids.clone()).await {
                    for (uuid, card) in result {
                        looked_up.insert(uuid, (card, system.name().to_string()));
                    }
                }
        }

        // Fall back: try every system for cards not yet resolved.
        let mut unfound: Vec<String> = rows
            .keys()
            .map(|(uuid, _)| uuid)
            .filter(|u| !looked_up.contains_key(*u))
            .cloned()
            .collect();
        unfound.sort_unstable();
        unfound.dedup();
        if !unfound.is_empty() {
            for system in retrievals {
                if let Ok(result) = system.get_cards_by_ids(unfound.clone()).await {
                    for (uuid, card) in result {
                        looked_up.entry(uuid).or_insert_with(|| (card, system.name().to_string()));
                    }
                }
            }
        }

        // (sort key, record)
        let mut records: Vec<([String; 4], Vec<String>)> = rows
            .iter()
            .filter_map(|((uuid, finish), row)| {
                use models::CardTrait as _;
                let (card, provider) = looked_up.get(uuid)?;
                let (set, number) = (card.get_set(), card.get_collector_number());
                let record = layout
                    .columns
                    .iter()
                    .map(|column| match column.field {
                        CsvField::SetCode => set.clone(),
                        CsvField::CollectorNumber => number.clone(),
                        CsvField::Quantity => row.quantity.to_string(),
                        CsvField::FoilQuantity => row.foil_quantity.to_string(),
                        CsvField::Provider => provider.clone(),
                        CsvField::Finish => finish.clone(),
                        CsvField::WantQuantity => row.want_quantity.to_string(),
                        CsvField::Name => card_name(card).to_string(),
                        CsvField::Id => uuid.clone(),
                    })
                    .collect();
                Some(([provider.clone(), set, number, finish.clone()], record))
            })
            .collect();
        // Stable output: by provider, set, collector number, finish.
        records.sort_unstable();

        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record(layout.columns.iter().map(|c| c.header.as_str()))?;
        for (_, record) in &records {
            wtr.write_record(record)?;
        }
        let data = String::from_utf8(wtr.into_inner()?)?;
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use retrieval::MagicSQLiteRetrievalSystem;

    use super::*;
    use crate::SQLitePersistenceSystem;

    #[tokio::test]
    async fn migrations_csv_import_export() {
        // Test File:
        // Set,CollectorNumber,Quantity,FoilQuantity
        // M13,39,2,1
        // ISD,173,0,4

        let (sender, receiver) = tokio::sync::watch::channel(0.0);

        let mut s = PersistenceSystem::SQLitePersistenceSystem(
            SQLitePersistenceSystem::new(true, None).unwrap(),
        );
        let r = RetrievalSystem::MagicSQLiteRetrievalSystem(
            MagicSQLiteRetrievalSystem::new(None, None).unwrap(),
        );
        s.import_csv(
            "../data/test.csv".to_string(),
            "New Collection".to_string(),
            &[r.clone()],
            Some(sender),
            &CsvLayout::default(),
        )
        .await
        .unwrap();

        let collections = s.list_collections(None).await.unwrap();
        assert_eq!(collections.len(), 2); // Default and the new one
        let new_collection = collections.iter().find(|c| !"Default".eq(*c)).unwrap();

        // 3 rows: M13/39 gets a normal row (qty 2) + a foil row (qty 1);
        // ISD/173 gets only a foil row (qty 4) since its normal qty is 0.
        let card_count = s
            .get_cards_in_collection_count(new_collection.clone(), &[], None)
            .await
            .unwrap();
        assert_eq!(card_count, 3);

        let cards = s
            .get_cards_in_collection_paginated(new_collection, CollectionCardsParams::new(0, 10))
            .await
            .unwrap();

        let normal = cards
            .iter()
            .find(|c| c.uuid == "0005d268-3fd0-5424-bc6b-573ecd713aa1" && c.finish.is_empty())
            .unwrap();
        assert_eq!(normal.quantity, 2);
        let foil = cards
            .iter()
            .find(|c| c.uuid == "0005d268-3fd0-5424-bc6b-573ecd713aa1" && c.finish == "foil")
            .unwrap();
        assert_eq!(foil.quantity, 1);

        assert!(!cards.iter().any(|c| c.uuid == "0003caab-9ff5-5d1a-bc06-976dd0457f19" && c.finish.is_empty()));
        let foil = cards
            .iter()
            .find(|c| c.uuid == "0003caab-9ff5-5d1a-bc06-976dd0457f19" && c.finish == "foil")
            .unwrap();
        assert_eq!(foil.quantity, 4);

        let latest_progress_update = receiver.borrow();
        assert_eq!(*latest_progress_update, 1.0);

        let export = s
            .export_collection(new_collection, &[r], &CsvLayout::default())
            .await
            .expect("Should work");

        println!("{export}");
        let provider = "MagicSQLite";
        assert_eq!(
            export,
            format!(
                "Set,CollectorNumber,Quantity,FoilQuantity,Provider,Finish,WantQuantity,Id\n\
                 ISD,173,0,4,{provider},,0,0003caab-9ff5-5d1a-bc06-976dd0457f19\n\
                 M13,39,2,1,{provider},,0,0005d268-3fd0-5424-bc6b-573ecd713aa1\n"
            )
        );
    }

    #[tokio::test]
    async fn test_export_with_custom_field_mapping() {
        let mut s = PersistenceSystem::SQLitePersistenceSystem(
            SQLitePersistenceSystem::new(true, None).unwrap(),
        );
        let r = RetrievalSystem::MagicSQLiteRetrievalSystem(
            MagicSQLiteRetrievalSystem::new(None, None).unwrap(),
        );
        s.import_csv(
            "../data/test.csv".to_string(),
            "New Collection".to_string(),
            &[r.clone()],
            None,
            &CsvLayout::default(),
        )
        .await
        .unwrap();
        let collection = s.list_collections(None).await.unwrap().into_iter().find(|c| c != "Default").unwrap();

        // Picked fields, in a chosen order, under chosen headers.
        let layout = CsvLayout::new([
            (CsvField::Name, "Card"),
            (CsvField::CollectorNumber, "Number"),
            (CsvField::SetCode, "Code"),
            (CsvField::Quantity, "Qty"),
            (CsvField::FoilQuantity, "Foils"),
        ]);
        let export = s.export_collection(&collection, &[r], &layout).await.unwrap();
        assert_eq!(
            export,
            "Card,Number,Code,Qty,Foils\nCaravan Vigil,173,ISD,0,4\nWar Priest of Thune,39,M13,2,1\n"
        );
    }

    #[tokio::test]
    async fn test_import_with_custom_field_mapping() {
        // A file using entirely different column names than gathers' own
        // default format — only resolvable because a mapping says what
        // each of these columns actually is.
        let path = std::env::temp_dir().join(format!("gathers_test_{}.csv", uuid::Uuid::new_v4()));
        std::fs::write(&path, "Code,Number,Qty,FoilQty\nM13,39,3,2\n").unwrap();

        let mapping = CsvLayout::new([
            (CsvField::SetCode, "Code"),
            (CsvField::CollectorNumber, "Number"),
            (CsvField::Quantity, "Qty"),
            (CsvField::FoilQuantity, "FoilQty"),
        ]);

        let mut s = PersistenceSystem::SQLitePersistenceSystem(
            SQLitePersistenceSystem::new(true, None).unwrap(),
        );
        let r = RetrievalSystem::MagicSQLiteRetrievalSystem(
            MagicSQLiteRetrievalSystem::new(None, None).unwrap(),
        );
        s.import_csv(
            path.to_string_lossy().to_string(),
            "Mapped Import".to_string(),
            &[r],
            None,
            &mapping,
        )
        .await
        .unwrap();
        std::fs::remove_file(&path).ok();

        let collection = s.list_collections(None).await.unwrap().into_iter().find(|c| c != "Default").unwrap();
        let cards = s
            .get_cards_in_collection_paginated(&collection, CollectionCardsParams::new(0, 10))
            .await
            .unwrap();
        let normal = cards
            .iter()
            .find(|c| c.uuid == "0005d268-3fd0-5424-bc6b-573ecd713aa1" && c.finish.is_empty())
            .unwrap();
        assert_eq!(normal.quantity, 3);
        let foil = cards
            .iter()
            .find(|c| c.uuid == "0005d268-3fd0-5424-bc6b-573ecd713aa1" && c.finish == "foil")
            .unwrap();
        assert_eq!(foil.quantity, 2);
    }

    #[tokio::test]
    async fn test_import_missing_required_column_errors() {
        let path = std::env::temp_dir().join(format!("gathers_test_{}.csv", uuid::Uuid::new_v4()));
        // No Quantity/FoilQuantity columns at all.
        std::fs::write(&path, "Set,CollectorNumber\nM13,39\n").unwrap();

        let mut s = PersistenceSystem::SQLitePersistenceSystem(
            SQLitePersistenceSystem::new(true, None).unwrap(),
        );
        let r = RetrievalSystem::MagicSQLiteRetrievalSystem(
            MagicSQLiteRetrievalSystem::new(None, None).unwrap(),
        );
        let result = s
            .import_csv(
                path.to_string_lossy().to_string(),
                "Broken Import".to_string(),
                &[r],
                None,
                &CsvLayout::default(),
            )
            .await;
        std::fs::remove_file(&path).ok();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Quantity"));
    }

    /// Imports `csv` into a fresh in-memory store under the test MTG DB,
    /// returning the store and the import result.
    async fn import_str(csv: &str) -> (PersistenceSystem, eyre::Result<()>) {
        import_str_with(csv, &CsvLayout::default()).await
    }

    async fn import_str_with(csv: &str, layout: &CsvLayout) -> (PersistenceSystem, eyre::Result<()>) {
        let path = std::env::temp_dir().join(format!("gathers_test_{}.csv", uuid::Uuid::new_v4()));
        std::fs::write(&path, csv).unwrap();
        let mut s = PersistenceSystem::SQLitePersistenceSystem(
            SQLitePersistenceSystem::new(true, None).unwrap(),
        );
        let r = RetrievalSystem::MagicSQLiteRetrievalSystem(
            MagicSQLiteRetrievalSystem::new(None, None).unwrap(),
        );
        let result = s
            .import_csv(
                path.to_string_lossy().to_string(),
                "Imported".to_string(),
                &[r],
                None,
                layout,
            )
            .await;
        std::fs::remove_file(&path).ok();
        (s, result)
    }

    #[tokio::test]
    async fn test_import_sums_duplicate_rows() {
        let (s, result) = import_str("Set,CollectorNumber,Quantity,FoilQuantity\nM13,39,2,0\nM13,39,5,1\n").await;
        result.unwrap();
        let cards = s
            .get_cards_in_collection_paginated(&"Imported".to_string(), CollectionCardsParams::new(0, 10))
            .await
            .unwrap();
        let normal = cards.iter().find(|c| c.finish.is_empty()).unwrap();
        assert_eq!(normal.quantity, 7);
        let foil = cards.iter().find(|c| c.finish == "foil").unwrap();
        assert_eq!(foil.quantity, 1);
    }

    #[tokio::test]
    async fn test_import_rejects_quantity_out_of_range() {
        let (s, result) = import_str("Set,CollectorNumber,Quantity,FoilQuantity\nM13,39,3000000000,0\n").await;
        let err = result.unwrap_err();
        assert!(matches!(err.downcast_ref::<PersistenceError>(), Some(PersistenceError::InvalidInput(_))));
        assert!(!s.list_collections(None).await.unwrap().contains(&"Imported".to_string()));
    }

    #[tokio::test]
    async fn test_import_rejects_unknown_provider() {
        let (_, result) = import_str("Set,CollectorNumber,Quantity,FoilQuantity,Provider\nM13,39,1,0,MagicSQLit\n").await;
        let err = result.unwrap_err();
        assert!(matches!(err.downcast_ref::<PersistenceError>(), Some(PersistenceError::InvalidInput(_))));
        assert!(err.to_string().contains("MagicSQLit"));
    }

    #[test]
    fn test_merge_duplicate_rows_saturates() {
        let row = |q| CSVCard {
            set_code: "M13".into(),
            collector_number: "39".into(),
            quantity: q,
            foil_quantity: 0,
            provider: String::new(),
            finish: String::new(),
            want_quantity: 0,
            id: String::new(),
        };
        let merged = merge_duplicate_rows(vec![row(i32::MAX as u32), row(10)]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].quantity, i32::MAX as u32);
    }

    /// Every row survives a round trip: wishlist-only cards, and finishes
    /// other than the default and `foil`.
    #[tokio::test]
    async fn test_export_import_round_trips_finishes_and_wants() {
        const A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1"; // M13 39
        const B: &str = "0003caab-9ff5-5d1a-bc06-976dd0457f19"; // ISD 173
        let mut s = PersistenceSystem::SQLitePersistenceSystem(SQLitePersistenceSystem::new(true, None).unwrap());
        let r = RetrievalSystem::MagicSQLiteRetrievalSystem(MagicSQLiteRetrievalSystem::new(None, None).unwrap());
        let col = s.add_collection("Source".to_string()).await.unwrap();
        let p = "MagicSQLite";
        let t = "2026-01-01T00:00:00Z";
        s.add_card_to_collection(&col, &A.to_string(), "", 2, t, p).await.unwrap();
        s.add_card_to_collection(&col, &A.to_string(), "foil", 1, t, p).await.unwrap();
        s.add_card_to_collection(&col, &A.to_string(), "etched", 3, t, p).await.unwrap();
        s.adjust_want_quantity(&col, &B.to_string(), 2, p).await.unwrap();

        let export = s.export_collection(&col, std::slice::from_ref(&r), &CsvLayout::default()).await.unwrap();
        assert_eq!(
            export,
            format!(
                "Set,CollectorNumber,Quantity,FoilQuantity,Provider,Finish,WantQuantity,Id\n\
                 ISD,173,0,0,{p},,2,{B}\n\
                 M13,39,2,1,{p},,0,{A}\n\
                 M13,39,3,0,{p},etched,0,{A}\n"
            )
        );

        let path = std::env::temp_dir().join(format!("gathers_test_{}.csv", uuid::Uuid::new_v4()));
        std::fs::write(&path, &export).unwrap();
        let result = s
            .import_csv(path.to_string_lossy().to_string(), "Copy".to_string(), &[r], None, &CsvLayout::default())
            .await;
        std::fs::remove_file(&path).ok();
        result.unwrap();

        let rows = |name: &str| {
            let s = s.clone();
            let name = name.to_string();
            async move {
                let mut rows: Vec<_> = s
                    .get_cards_in_collection_paginated(&name, CollectionCardsParams::new(0, 100))
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|c| (c.uuid, c.finish, c.quantity, c.want_quantity, c.provider))
                    .collect();
                rows.sort();
                rows
            }
        };
        assert_eq!(rows("Copy").await, rows("Source").await);
        assert_eq!(rows("Copy").await.len(), 4);
    }

    async fn imported_rows(s: &PersistenceSystem) -> Vec<(String, String, i32, i32)> {
        let mut rows: Vec<_> = s
            .get_cards_in_collection_paginated(&"Imported".to_string(), CollectionCardsParams::new(0, 100))
            .await
            .unwrap()
            .into_iter()
            .map(|c| (c.uuid, c.finish, c.quantity, c.want_quantity))
            .collect();
        rows.sort();
        rows
    }

    const WAR_PRIEST: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1"; // M13 39
    const CARAVAN_VIGIL: &str = "0003caab-9ff5-5d1a-bc06-976dd0457f19"; // ISD 173

    /// A preset without `FoilQuantity` puts foil copies on rows of their own.
    #[tokio::test]
    async fn test_export_tcgplayer_preset_splits_foils() {
        let (s, result) = import_str("Set,CollectorNumber,Quantity,FoilQuantity\nM13,39,2,1\n").await;
        result.unwrap();
        let r = RetrievalSystem::MagicSQLiteRetrievalSystem(MagicSQLiteRetrievalSystem::new(None, None).unwrap());
        let export = s
            .export_collection(&"Imported".to_string(), &[r], &CsvLayout::preset("TCGPlayer").unwrap())
            .await
            .unwrap();
        assert_eq!(
            export,
            "Quantity,Name,Set Code,Card Number,Printing\n\
             2,War Priest of Thune,M13,39,\n\
             1,War Priest of Thune,M13,39,foil\n"
        );
    }

    /// Other formats' column names and finish spellings, mapped by a
    /// layout; columns the layout doesn't mention are ignored.
    #[tokio::test]
    async fn test_import_with_preset_layout_and_finish_spellings() {
        let csv = "Quantity,Name,Simple Name,Set Code,Card Number,Printing,Condition\n\
                   2,War Priest of Thune,x,M13,39,Normal,NM\n\
                   1,War Priest of Thune,x,M13,39,Foil,NM\n\
                   ,Caravan Vigil,x,ISD,173,Foil,NM\n";
        let (s, result) = import_str_with(csv, &CsvLayout::preset("tcgplayer").unwrap()).await;
        result.unwrap();
        assert_eq!(
            imported_rows(&s).await,
            vec![(WAR_PRIEST.into(), "".into(), 2, 0), (WAR_PRIEST.into(), "foil".into(), 1, 0)]
        );
    }

    /// An `Id` finds the card exactly, even when set and number are wrong
    /// or missing; an unknown id falls back to set and number.
    #[tokio::test]
    async fn test_import_by_id() {
        let csv = format!(
            "Id,Set,CollectorNumber,WantQuantity\n{WAR_PRIEST},ISD,173,2\nnot-an-id,ISD,173,1\n"
        );
        let layout = CsvLayout::new([
            (CsvField::Id, "Id"),
            (CsvField::SetCode, "Set"),
            (CsvField::CollectorNumber, "CollectorNumber"),
            (CsvField::WantQuantity, "WantQuantity"),
        ]);
        let (s, result) = import_str_with(&csv, &layout).await;
        result.unwrap();
        assert_eq!(
            imported_rows(&s).await,
            vec![(CARAVAN_VIGIL.into(), "".into(), 0, 1), (WAR_PRIEST.into(), "".into(), 0, 2)]
        );

        // Id alone is enough to find cards.
        let layout = CsvLayout::new([(CsvField::Id, "Id"), (CsvField::Quantity, "Quantity")]);
        let (s, result) = import_str_with(&format!("Id,Quantity\n{CARAVAN_VIGIL},3\n"), &layout).await;
        result.unwrap();
        assert_eq!(imported_rows(&s).await, vec![(CARAVAN_VIGIL.into(), "".into(), 3, 0)]);
    }

    #[tokio::test]
    async fn test_import_needs_a_way_to_find_cards_and_a_count() {
        let (_, result) = import_str("Set,Quantity\nM13,1\n").await;
        assert!(result.unwrap_err().to_string().contains("'CollectorNumber'"));
        let (_, result) = import_str("Set,CollectorNumber,Name\nM13,39,x\n").await;
        assert!(result.unwrap_err().to_string().contains("no quantity column"));
    }

    #[test]
    fn test_layout_validation() {
        assert!(CsvLayout::gathers().validate().is_ok());
        for preset in CsvLayout::presets() {
            assert!(preset.layout.validate().is_ok(), "{}", preset.id);
        }
        assert!(CsvLayout::new([]).validate().is_err());
        assert!(CsvLayout::new([(CsvField::Name, "A"), (CsvField::Id, "A")]).validate().is_err());
        assert!(CsvLayout::new([(CsvField::Name, "A"), (CsvField::Name, "B")]).validate().is_err());
        assert!(CsvLayout::new([(CsvField::Name, " ")]).validate().is_err());
        assert_eq!(CsvLayout::preset("Card-Kingdom"), CsvLayout::preset("cardkingdom"));
        assert_eq!(CsvLayout::preset("goldfish"), CsvLayout::preset("mtggoldfish"));
        assert!(CsvLayout::preset("nope").is_none());
    }
}
