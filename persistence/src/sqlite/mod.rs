mod cards;
mod collections;
mod purchase_history;
mod share_links;
#[cfg(test)]
mod tests;

use include_dir::{Dir, include_dir};
use models::CardID;
use models::CollectionID;
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::Mutex;

use crate::{CollectionCard, CollectionCardsParams, PersistenceSystemTrait, PurchaseHistoryEntry, PurchaseSummary, ShareLink, UpdateEntryResult};

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");
static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).expect("failed to load DB migrations from embedded directory"));

#[derive(Debug, Clone)]
pub struct SQLitePersistenceSystem {
    connection: Arc<Mutex<Connection>>,
}

impl SQLitePersistenceSystem {
    pub fn new(in_memory: bool, db_path: Option<String>) -> eyre::Result<Self> {
        let mut conn = if in_memory {
            Connection::open(":memory:")?
        } else {
            let path = db_path.unwrap_or_else(|| "storage.db".to_string());
            let path = std::path::Path::new(&path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            Connection::open(path)?
        };
        MIGRATIONS.to_latest(&mut conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
        })
    }
}

impl PersistenceSystemTrait for SQLitePersistenceSystem {
    async fn add_collection(&mut self, name: CollectionID) -> eyre::Result<CollectionID> {
        let conn = self.connection.lock().await;
        collections::add_collection(&conn, &name)
    }

    async fn remove_collection(
        &mut self,
        name: &CollectionID,
        move_to: Option<CollectionID>,
    ) -> eyre::Result<CollectionID> {
        let conn = self.connection.lock().await;
        collections::remove_collection(&conn, name, move_to.as_ref())
    }

    async fn rename_collection(
        &mut self,
        old_name: &CollectionID,
        new_name: &CollectionID,
    ) -> eyre::Result<()> {
        let conn = self.connection.lock().await;
        collections::rename_collection(&conn, old_name, new_name)
    }

    async fn list_collections(&self, filter: Option<String>) -> eyre::Result<Vec<CollectionID>> {
        let conn = self.connection.lock().await;
        collections::list_collections(&conn, filter.as_deref())
    }

    async fn get_cards_in_collection_count(
        &self,
        collection_id: CollectionID,
        providers: &[String],
    ) -> eyre::Result<usize> {
        let conn = self.connection.lock().await;
        collections::get_cards_count(&conn, &collection_id, providers)
    }

    async fn add_card_to_collection(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        quantity: i32,
        foil_quantity: i32,
        time_added: &str,
        provider: &str,
    ) -> eyre::Result<CollectionCard> {
        let mut conn = self.connection.lock().await;
        let tx = conn.transaction()?;
        let mut result = cards::add_cards(
            &tx,
            collection_id,
            &[CollectionCard {
                uuid: card_uuid.clone(),
                collection: collection_id.clone(),
                quantity,
                foil_quantity,
                want_quantity: 0,
                time_added: time_added.to_string(),
                provider: provider.to_string(),
            }],
        )?;
        let card = result
            .pop()
            .ok_or_else(|| eyre::eyre!("No card returned from upsert"))?;
        purchase_history::trim_history_to_collection(
            &tx,
            collection_id,
            card_uuid,
            card.quantity,
            card.foil_quantity,
        )?;
        tx.commit()?;
        Ok(card)
    }

    async fn add_cards_to_collection(
        &mut self,
        collection_id: &CollectionID,
        input_cards: &[CollectionCard],
    ) -> eyre::Result<Vec<CollectionCard>> {
        let mut conn = self.connection.lock().await;
        let tx = conn.transaction()?;
        let result = cards::add_cards(&tx, collection_id, input_cards)?;
        for card in &result {
            purchase_history::trim_history_to_collection(
                &tx,
                collection_id,
                &card.uuid,
                card.quantity,
                card.foil_quantity,
            )?;
        }
        tx.commit()?;
        Ok(result)
    }

    async fn set_want_quantity(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        want_quantity: i32,
        provider: &str,
    ) -> eyre::Result<CollectionCard> {
        let conn = self.connection.lock().await;
        let now = chrono::Utc::now().to_rfc3339();
        cards::set_want_quantity(&conn, collection_id, card_uuid, want_quantity, provider, &now)
    }

    async fn move_cards_between_collections(
        &mut self,
        input_cards: &[CollectionCard],
        to_collection_id: CollectionID,
    ) -> eyre::Result<()> {
        let mut conn = self.connection.lock().await;
        let tx = conn.transaction()?;
        for c in input_cards {
            if c.quantity == 0 && c.foil_quantity == 0 {
                continue;
            }
            if c.collection == to_collection_id {
                continue;
            }
            let source_results = cards::add_cards(
                &tx,
                &c.collection,
                &[CollectionCard {
                    uuid: c.uuid.clone(),
                    collection: c.collection.clone(),
                    quantity: -c.quantity,
                    foil_quantity: -c.foil_quantity,
                    want_quantity: 0,
                    time_added: c.time_added.clone(),
                    provider: c.provider.clone(),
                }],
            )?;
            let (src_qty, src_foil_qty) = source_results
                .first()
                .map(|sc| (sc.quantity, sc.foil_quantity))
                .unwrap_or((0, 0));
            purchase_history::transfer_trimmed_history_to_collection(
                &tx,
                &c.collection,
                &to_collection_id,
                &c.uuid,
                src_qty,
                src_foil_qty,
            )?;
            let provider = source_results
                .first()
                .filter(|sc| !sc.provider.is_empty())
                .map(|sc| sc.provider.clone())
                .unwrap_or_else(|| c.provider.clone());
            cards::add_cards(
                &tx,
                &to_collection_id,
                &[CollectionCard {
                    uuid: c.uuid.clone(),
                    collection: to_collection_id.clone(),
                    quantity: c.quantity,
                    foil_quantity: c.foil_quantity,
                    want_quantity: 0,
                    time_added: c.time_added.clone(),
                    provider,
                }],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn get_cards_in_collection_paginated(
        &self,
        collection_id: &CollectionID,
        params: CollectionCardsParams,
    ) -> eyre::Result<Vec<CollectionCard>> {
        let conn = self.connection.lock().await;
        cards::get_paginated(&conn, collection_id, params)
    }

    async fn record_purchase(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        quantity: i32,
        foil_quantity: i32,
        normal_price_per_unit: Option<f64>,
        foil_price_per_unit: Option<f64>,
        provider: &str,
        recorded_at: &str,
    ) -> eyre::Result<()> {
        let conn = self.connection.lock().await;
        purchase_history::record_purchase(
            &conn,
            collection_id,
            card_uuid,
            quantity,
            foil_quantity,
            normal_price_per_unit,
            foil_price_per_unit,
            provider,
            recorded_at,
        )
    }

    async fn get_purchase_history(
        &self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
    ) -> eyre::Result<Vec<PurchaseHistoryEntry>> {
        let conn = self.connection.lock().await;
        purchase_history::get_history(&conn, collection_id, card_uuid)
    }

    async fn get_all_purchase_history(
        &self,
        collection_id: &CollectionID,
    ) -> eyre::Result<Vec<PurchaseHistoryEntry>> {
        let conn = self.connection.lock().await;
        purchase_history::get_all_history(&conn, collection_id)
    }

    async fn get_collection_purchase_totals(
        &self,
        collection_id: &CollectionID,
    ) -> eyre::Result<std::collections::HashMap<CardID, PurchaseSummary>> {
        let conn = self.connection.lock().await;
        purchase_history::get_collection_totals(&conn, collection_id)
    }

    async fn delete_purchase_entry(
        &mut self,
        collection_id: &CollectionID,
        entry_id: i64,
    ) -> eyre::Result<bool> {
        let conn = self.connection.lock().await;
        purchase_history::delete_entry(&conn, collection_id, entry_id)
    }

    async fn update_purchase_entry(
        &mut self,
        collection_id: &CollectionID,
        entry_id: i64,
        quantity: i32,
        foil_quantity: i32,
        normal_price_per_unit: Option<f64>,
        foil_price_per_unit: Option<f64>,
    ) -> eyre::Result<UpdateEntryResult> {
        let conn = self.connection.lock().await;
        purchase_history::update_entry(&conn, collection_id, entry_id, quantity, foil_quantity, normal_price_per_unit, foil_price_per_unit)
    }

    async fn create_share_link(&mut self, collection_id: &CollectionID) -> eyre::Result<ShareLink> {
        let conn = self.connection.lock().await;
        share_links::create(&conn, collection_id)
    }

    async fn list_share_links(&self, collection_id: &CollectionID) -> eyre::Result<Vec<ShareLink>> {
        let conn = self.connection.lock().await;
        share_links::list(&conn, collection_id)
    }

    async fn revoke_share_link(&mut self, collection_id: &CollectionID, token: &str) -> eyre::Result<bool> {
        let conn = self.connection.lock().await;
        share_links::revoke(&conn, collection_id, token)
    }

    async fn resolve_share_link(&self, token: &str) -> eyre::Result<Option<CollectionID>> {
        let conn = self.connection.lock().await;
        share_links::resolve(&conn, token)
    }
}
