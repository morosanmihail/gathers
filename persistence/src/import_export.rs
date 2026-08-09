use std::collections::HashMap;

use retrieval::{NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait as _};

use crate::{CollectionCard, CollectionCardsParams, PersistenceSystem, PersistenceSystemTrait as _};
use crate::csv_models::CSVCard;

fn systems_by_name<'a>(retrievals: &'a [RetrievalSystem]) -> HashMap<&'a str, &'a RetrievalSystem> {
    retrievals.iter().map(|r| (r.name(), r)).collect()
}

impl PersistenceSystem {
    pub async fn import_csv(
        &mut self,
        filename: String,
        collection_name: String,
        retrievals: &[RetrievalSystem],
        progress_sender: Option<tokio::sync::watch::Sender<f32>>,
    ) -> eyre::Result<()> {
        const DEFAULT_PROVIDER: &str = "MagicSQLite";
        const BULK_CHUNK_SIZE: usize = 500;

        let mut rdr = csv::Reader::from_path(filename)?;
        let mut cards: Vec<CSVCard> = vec![];
        for result in rdr.deserialize() {
            cards.push(result?);
        }

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
            let batch: Vec<CollectionCard> = g
                .iter()
                .map(|c| CollectionCard {
                    uuid: c.0.clone(),
                    quantity: c.1 as i32,
                    foil_quantity: c.2 as i32,
                    want_quantity: 0,
                    collection: collection_id.clone(),
                    time_added: time_added.clone(),
                    provider: c.3.clone(),
                })
                .collect();
            self.add_cards_to_collection(&collection_id, &batch).await?;

            i += batch.len() as f32;
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
    ) -> eyre::Result<String> {
        let by_name = systems_by_name(retrievals);

        let mut wtr = csv::Writer::from_writer(vec![]);
        let mut offset = 0;
        let limit = 100;
        loop {
            let cards = self
                .get_cards_in_collection_paginated(collection_id, CollectionCardsParams::new(offset, limit))
                .await?;
            if cards.is_empty() {
                break;
            }

            // Group UUIDs by stored provider so we issue one lookup per system.
            let mut ids_by_provider: HashMap<&str, Vec<String>> = Default::default();
            for card in &cards {
                ids_by_provider
                    .entry(card.provider.as_str())
                    .or_default()
                    .push(card.uuid.clone());
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
            let unfound: Vec<String> = cards
                .iter()
                .filter(|c| !looked_up.contains_key(&c.uuid))
                .map(|c| c.uuid.clone())
                .collect();
            if !unfound.is_empty() {
                for system in retrievals {
                    if let Ok(result) = system.get_cards_by_ids(unfound.clone()).await {
                        for (uuid, card) in result {
                            looked_up
                                .entry(uuid)
                                .or_insert_with(|| (card, system.name().to_string()));
                        }
                    }
                }
            }

            for card in &cards {
                if let Some((searched, provider)) = looked_up.get(&card.uuid) {
                    use models::CardTrait as _;
                    wtr.serialize(CSVCard {
                        set_code: searched.get_set(),
                        collector_number: searched.get_collector_number(),
                        quantity: card.quantity as u32,
                        foil_quantity: card.foil_quantity as u32,
                        provider: provider.clone(),
                    })?;
                }
            }

            offset += limit;
            wtr.flush()?;
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
        s.import_csv("../data/test.csv".to_string(), "New Collection".to_string(), &[r.clone()], Some(sender))
            .await
            .unwrap();

        let collections = s.list_collections(None).await.unwrap();
        assert_eq!(collections.len(), 2); // Default and the new one
        let new_collection = collections.iter().find(|c| !"Default".eq(*c)).unwrap();

        let card_count = s
            .get_cards_in_collection_count(new_collection.clone(), &[])
            .await
            .unwrap();
        assert_eq!(card_count, 2);

        let cards = s
            .get_cards_in_collection_paginated(new_collection, CollectionCardsParams::new(0, 10))
            .await
            .unwrap();

        let card = cards
            .iter()
            .find(|c| c.uuid == "0005d268-3fd0-5424-bc6b-573ecd713aa1")
            .unwrap();
        assert_eq!(card.quantity, 2);
        assert_eq!(card.foil_quantity, 1);

        let card = cards
            .iter()
            .find(|c| c.uuid == "0003caab-9ff5-5d1a-bc06-976dd0457f19")
            .unwrap();
        assert_eq!(card.quantity, 0);
        assert_eq!(card.foil_quantity, 4);

        let latest_progress_update = receiver.borrow();
        assert_eq!(*latest_progress_update, 1.0);

        let export = s
            .export_collection(new_collection, &[r])
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
}
