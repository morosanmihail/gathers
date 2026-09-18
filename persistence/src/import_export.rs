use std::collections::HashMap;

use retrieval::{NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait as _};

use crate::{CollectionCard, CollectionCardsParams, PersistenceSystem, PersistenceSystemTrait as _};
use crate::csv_models::{CSVCard, CsvField, CsvFieldMapping};

fn systems_by_name<'a>(retrievals: &'a [RetrievalSystem]) -> HashMap<&'a str, &'a RetrievalSystem> {
    retrievals.iter().map(|r| (r.name(), r)).collect()
}

/// Reads every row of `filename` into `CSVCard`s, resolving each column by
/// the header text `mapping` says holds it (falling back to
/// `CsvField::default_header` for anything not overridden) rather than
/// assuming gathers' own fixed column names. `SetCode`/`CollectorNumber`/
/// `Quantity` must all be present under their resolved header; `Provider`
/// and `FoilQuantity` are optional and default to an empty string / 0 when
/// the file has no such column at all — several third-party formats (see
/// `CsvFieldMapping::preset`) don't track foil as a separate count.
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
                    .parse()
                    .map_err(|_| eyre::eyre!("invalid value {v:?} in '{}' column", mapping.header_for(field))),
                None => Ok(0),
            }
        };
        cards.push(CSVCard {
            set_code: get(CsvField::SetCode).unwrap_or_default().to_string(),
            collector_number: get(CsvField::CollectorNumber).unwrap_or_default().to_string(),
            quantity: parse_count(CsvField::Quantity)?,
            foil_quantity: parse_count(CsvField::FoilQuantity)?,
            provider: get(CsvField::Provider).unwrap_or_default().to_string(),
        });
    }
    Ok(cards)
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

        let cards = read_csv(&filename, mapping)?;

        let by_name = systems_by_name(retrievals);

        // Group cards by provider, treating an empty provider as DEFAULT_PROVIDER.
        let mut groups: HashMap<&str, Vec<&CSVCard>> = Default::default();
        for card in &cards {
            let provider = if card.provider.is_empty() {
                DEFAULT_PROVIDER
            } else {
                card.provider.as_str()
            };
            groups.entry(provider).or_default().push(card);
        }

        // Resolve each group against its retrieval system, falling back to the
        // first available system when the named provider is not configured.
        // (uuid, quantity, foil_quantity, provider_name)
        let mut cta: Vec<(String, u32, u32, String)> = vec![];
        for (provider, group) in &groups {
            let system = by_name
                .get(provider)
                .copied()
                .or_else(|| retrievals.first())
                .ok_or_else(|| eyre::eyre!("No retrieval system available for import"))?;

            let input: Vec<(String, String)> = group
                .iter()
                .map(|c| (c.set_code.clone(), c.collector_number.clone()))
                .collect();

            let mut resolved = vec![];
            for chunk in input.chunks(BULK_CHUNK_SIZE) {
                resolved.extend(system.bulk_search_cards(chunk.to_vec()).await?);
            }

            for (set_code, collector_number, uuid) in resolved {
                if let Some(c) = group
                    .iter()
                    .find(|c| c.set_code == set_code && c.collector_number == collector_number)
                {
                    cta.push((uuid, c.quantity, c.foil_quantity, system.name().to_string()));
                }
            }
        }

        if cta.is_empty() {
            return Err(eyre::eyre!("No cards could be resolved from the CSV"));
        }

        let now = chrono::Utc::now();
        let time_added = now.to_rfc3339();
        let collection_id = self.add_collection(collection_name).await?;
        let total = cta.len() as f32;
        let mut i: f32 = 0.0;

        for g in cta.chunks(50) {
            // The CSV interchange format only understands two finishes —
            // the default (nonfoil) bucket and `foil` — so each resolved
            // row expands into up to two finish rows.
            let mut batch: Vec<CollectionCard> = vec![];
            for c in g {
                if c.1 > 0 {
                    batch.push(CollectionCard {
                        uuid: c.0.clone(),
                        finish: String::new(),
                        quantity: c.1 as i32,
                        want_quantity: 0,
                        collection: collection_id.clone(),
                        time_added: time_added.clone(),
                        provider: c.3.clone(),
                    });
                }
                if c.2 > 0 {
                    batch.push(CollectionCard {
                        uuid: c.0.clone(),
                        finish: "foil".to_string(),
                        quantity: c.2 as i32,
                        want_quantity: 0,
                        collection: collection_id.clone(),
                        time_added: time_added.clone(),
                        provider: c.3.clone(),
                    });
                }
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

        // Fetch every (uuid, finish) row first and group by uuid — the CSV
        // interchange format only has two quantity columns, so each card's
        // rows collapse into (quantity for the default finish, quantity for
        // `foil`). Any other finish (e.g. a Pokemon reverse-holo) isn't
        // representable in this two-column format and is left out of the
        // export entirely, rather than mislabeling it as foil.
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

        struct ExportRow {
            quantity: u32,
            foil_quantity: u32,
            provider: String,
        }
        let mut by_uuid: HashMap<String, ExportRow> = HashMap::new();
        for card in &all_cards {
            let entry = by_uuid.entry(card.uuid.clone()).or_insert_with(|| ExportRow {
                quantity: 0,
                foil_quantity: 0,
                provider: card.provider.clone(),
            });
            match card.finish.as_str() {
                "" if card.quantity > 0 => entry.quantity += card.quantity as u32,
                "foil" if card.quantity > 0 => entry.foil_quantity += card.quantity as u32,
                _ => {}
            }
        }
        // A card whose only rows are an unrepresentable finish (or a
        // wishlist-only `""` row with quantity 0) has nothing to export.
        by_uuid.retain(|_, row| row.quantity > 0 || row.foil_quantity > 0);

        // Group UUIDs by stored provider so we issue one lookup per system.
        let mut ids_by_provider: HashMap<&str, Vec<String>> = Default::default();
        for (uuid, row) in &by_uuid {
            ids_by_provider.entry(row.provider.as_str()).or_default().push(uuid.clone());
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
        let unfound: Vec<String> = by_uuid.keys().filter(|u| !looked_up.contains_key(*u)).cloned().collect();
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
        ])?;
        for (uuid, row) in &by_uuid {
            if let Some((searched, provider)) = looked_up.get(uuid) {
                use models::CardTrait as _;
                wtr.write_record([
                    searched.get_set(),
                    searched.get_collector_number(),
                    row.quantity.to_string(),
                    row.foil_quantity.to_string(),
                    provider.clone(),
                ])?;
            }
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
            .get_cards_in_collection_count(new_collection.clone(), &[])
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
        assert!(
            export
                == format!("Set,CollectorNumber,Quantity,FoilQuantity,Provider\nM13,39,2,1,{provider}\nISD,173,0,4,{provider}\n")
                || export
                    == format!("Set,CollectorNumber,Quantity,FoilQuantity,Provider\nISD,173,0,4,{provider}\nM13,39,2,1,{provider}\n")
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
        assert!(export.starts_with("Code,Number,Quantity,FoilQuantity,Provider\n"));
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
}
