use std::collections::HashMap;

use retrieval::{NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait as _};

use crate::{CollectionCard, CollectionCardsParams, PersistenceError, PersistenceSystem, PersistenceSystemTrait as _};
use crate::csv_models::{CSVCard, CsvField, CsvFieldMapping};

fn systems_by_name<'a>(retrievals: &'a [RetrievalSystem]) -> HashMap<&'a str, &'a RetrievalSystem> {
    retrievals.iter().map(|r| (r.name(), r)).collect()
}

/// Reads every row of `filename` into `CSVCard`s, resolving each column by
/// the header text `mapping` says holds it (falling back to
/// `CsvField::default_header` for anything not overridden) rather than
/// assuming gathers' own fixed column names. `SetCode`/`CollectorNumber`/
/// `Quantity` must all be present under their resolved header; the rest are
/// optional and default to an empty string / 0 when the file has no such
/// column at all — several third-party formats (see
/// `CsvFieldMapping::preset`) don't track foil as a separate count, and
/// files exported before `Finish`/`WantQuantity` existed have neither.
fn read_csv(filename: &str, mapping: &CsvFieldMapping) -> eyre::Result<Vec<CSVCard>> {
    let mut rdr = csv::Reader::from_path(filename)?;
    let header_to_field = mapping.header_to_field();

    let mut field_index: HashMap<CsvField, usize> = HashMap::new();
    for (i, header) in rdr.headers()?.iter().enumerate() {
        if let Some(&field) = header_to_field.get(header) {
            field_index.insert(field, i);
        }
    }

    for required in [CsvField::SetCode, CsvField::CollectorNumber, CsvField::Quantity] {
        if !field_index.contains_key(&required) {
            return Err(eyre::eyre!(
                "CSV is missing a '{}' column",
                mapping.header_for(required)
            ));
        }
    }

    let mut cards = vec![];
    for result in rdr.records() {
        let record = result?;
        let get = |field: CsvField| -> Option<&str> {
            field_index.get(&field).and_then(|&i| record.get(i))
        };
        // A field with no matching column at all defaults to 0 (rather than
        // failing to parse an empty string) — see the doc comment above.
        let parse_count = |field: CsvField| -> eyre::Result<u32> {
            match get(field) {
                Some(v) => v
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n <= i32::MAX as u32)
                    .ok_or_else(|| eyre::eyre!(
                        "invalid value {v:?} in '{}' column (expected a whole number from 0 to {})",
                        mapping.header_for(field),
                        i32::MAX
                    )),
                None => Ok(0),
            }
        };
        cards.push(CSVCard {
            set_code: get(CsvField::SetCode).unwrap_or_default().to_string(),
            collector_number: get(CsvField::CollectorNumber).unwrap_or_default().to_string(),
            quantity: parse_count(CsvField::Quantity)?,
            foil_quantity: parse_count(CsvField::FoilQuantity)?,
            provider: get(CsvField::Provider).unwrap_or_default().to_string(),
            finish: get(CsvField::Finish).unwrap_or_default().to_string(),
            want_quantity: parse_count(CsvField::WantQuantity)?,
        });
    }
    Ok(cards)
}

/// Folds rows naming the same card and finish (same provider, set, collector
/// number and finish) into one, summing their quantities — otherwise only the first such row
/// would be matched back to the resolved card and the rest silently lost.
/// Sums saturate at `i32::MAX`, the most a collection row can hold.
fn merge_duplicate_rows(cards: Vec<CSVCard>) -> Vec<CSVCard> {
    let mut merged: Vec<CSVCard> = Vec::with_capacity(cards.len());
    let mut index: HashMap<(String, String, String, String), usize> = HashMap::new();
    for card in cards {
        let key = (
            card.provider.clone(),
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

impl PersistenceSystem {
    pub async fn import_csv(
        &mut self,
        filename: String,
        collection_name: String,
        retrievals: &[RetrievalSystem],
        progress_sender: Option<tokio::sync::watch::Sender<f32>>,
        mapping: &CsvFieldMapping,
    ) -> eyre::Result<()> {
        const DEFAULT_PROVIDER: &str = "MagicSQLite";
        const BULK_CHUNK_SIZE: usize = 500;

        // Everything that can go wrong from here until the collection is
        // written is down to the file's contents, not storage.
        let invalid = |e: eyre::Report| PersistenceError::InvalidInput(format!("{e:#}"));

        let cards = merge_duplicate_rows(read_csv(&filename, mapping).map_err(invalid)?);

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

            // Rows for several finishes of one card share a lookup.
            let mut input: Vec<(String, String)> = group
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
                for c in group
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

    pub async fn export_collection(
        &self,
        collection_id: &models::CollectionID,
        retrievals: &[RetrievalSystem],
        mapping: &CsvFieldMapping,
    ) -> eyre::Result<String> {
        let by_name = systems_by_name(retrievals);

        // Fetch every (uuid, finish) row first. A card's default and `foil`
        // rows share one CSV row (`Quantity` / `FoilQuantity`, empty
        // `Finish`) — what exports looked like before other finishes were
        // supported, so they still import anywhere. Every other finish
        // (e.g. a Pokemon `1st Edition`) gets a row of its own, named in
        // `Finish`, with its count in `Quantity`.
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
        // Keyed by (uuid, CSV `Finish`).
        let mut rows: HashMap<(String, String), ExportRow> = HashMap::new();
        for card in &all_cards {
            let quantity = card.quantity.max(0) as u32;
            let want_quantity = card.want_quantity.max(0) as u32;
            let csv_finish = if card.finish == "foil" { "" } else { card.finish.as_str() };
            let entry = rows
                .entry((card.uuid.clone(), csv_finish.to_string()))
                .or_insert_with(|| ExportRow { provider: card.provider.clone(), ..Default::default() });
            if card.finish == "foil" {
                entry.foil_quantity += quantity;
            } else {
                entry.quantity += quantity;
            }
            entry.want_quantity += want_quantity;
        }
        rows.retain(|_, row| row.quantity > 0 || row.foil_quantity > 0 || row.want_quantity > 0);

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

        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record([
            mapping.header_for(CsvField::SetCode),
            mapping.header_for(CsvField::CollectorNumber),
            mapping.header_for(CsvField::Quantity),
            mapping.header_for(CsvField::FoilQuantity),
            mapping.header_for(CsvField::Provider),
            mapping.header_for(CsvField::Finish),
            mapping.header_for(CsvField::WantQuantity),
        ])?;
        let mut records: Vec<[String; 7]> = rows
            .iter()
            .filter_map(|((uuid, finish), row)| {
                use models::CardTrait as _;
                let (searched, provider) = looked_up.get(uuid)?;
                Some([
                    searched.get_set(),
                    searched.get_collector_number(),
                    row.quantity.to_string(),
                    row.foil_quantity.to_string(),
                    provider.clone(),
                    finish.clone(),
                    row.want_quantity.to_string(),
                ])
            })
            .collect();
        // Stable output: by provider, set, collector number, finish.
        records.sort_unstable_by(|a, b| (&a[4], &a[0], &a[1], &a[5]).cmp(&(&b[4], &b[0], &b[1], &b[5])));
        for record in &records {
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
            &CsvFieldMapping::default(),
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
            .export_collection(new_collection, &[r], &CsvFieldMapping::default())
            .await
            .expect("Should work");

        println!("{export}");
        let provider = "MagicSQLite";
        assert_eq!(
            export,
            format!("Set,CollectorNumber,Quantity,FoilQuantity,Provider,Finish,WantQuantity\nISD,173,0,4,{provider},,0\nM13,39,2,1,{provider},,0\n")
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
            &CsvFieldMapping::default(),
        )
        .await
        .unwrap();
        let collection = s.list_collections(None).await.unwrap().into_iter().find(|c| c != "Default").unwrap();

        let mut mapping = CsvFieldMapping::new();
        mapping.set(CsvField::SetCode, Some("Code".to_string()));
        mapping.set(CsvField::CollectorNumber, Some("Number".to_string()));

        let export = s.export_collection(&collection, &[r], &mapping).await.unwrap();

        // Overridden headers used for the mapped fields, defaults kept for the rest.
        assert!(export.starts_with("Code,Number,Quantity,FoilQuantity,Provider,Finish,WantQuantity\n"));
        assert!(export.contains("M13,39,2,1,"));
        assert!(export.contains("ISD,173,0,4,"));
    }

    #[tokio::test]
    async fn test_import_with_custom_field_mapping() {
        // A file using entirely different column names than gathers' own
        // default format — only resolvable because a mapping says what
        // each of these columns actually is.
        let path = std::env::temp_dir().join(format!("gathers_test_{}.csv", uuid::Uuid::new_v4()));
        std::fs::write(&path, "Code,Number,Qty,FoilQty\nM13,39,3,2\n").unwrap();

        let mut mapping = CsvFieldMapping::new();
        mapping.set(CsvField::SetCode, Some("Code".to_string()));
        mapping.set(CsvField::CollectorNumber, Some("Number".to_string()));
        mapping.set(CsvField::Quantity, Some("Qty".to_string()));
        mapping.set(CsvField::FoilQuantity, Some("FoilQty".to_string()));

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
                &CsvFieldMapping::default(),
            )
            .await;
        std::fs::remove_file(&path).ok();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Quantity"));
    }

    /// Imports `csv` into a fresh in-memory store under the test MTG DB,
    /// returning the store and the import result.
    async fn import_str(csv: &str) -> (PersistenceSystem, eyre::Result<()>) {
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
                &CsvFieldMapping::default(),
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

        let export = s.export_collection(&col, std::slice::from_ref(&r), &CsvFieldMapping::default()).await.unwrap();
        assert_eq!(
            export,
            format!(
                "Set,CollectorNumber,Quantity,FoilQuantity,Provider,Finish,WantQuantity\n\
                 ISD,173,0,0,{p},,2\n\
                 M13,39,2,1,{p},,0\n\
                 M13,39,3,0,{p},etched,0\n"
            )
        );

        let path = std::env::temp_dir().join(format!("gathers_test_{}.csv", uuid::Uuid::new_v4()));
        std::fs::write(&path, &export).unwrap();
        let result = s
            .import_csv(path.to_string_lossy().to_string(), "Copy".to_string(), &[r], None, &CsvFieldMapping::default())
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
}
