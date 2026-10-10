//! A collection's value over time and per card, built on price history (see
//! `ServerConfig::price_history_enabled`). Only cards currently owned count:
//! each is valued, on every day, at the retailer it's valued at today, so
//! these line up with `/value_breakdown`.

use std::collections::{BTreeSet, HashMap};

use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::NaiveDate;
use persistence::{DailyPrice, PersistenceSystemTrait as _};

use super::{CollectionPricing, UnitPrices, hydrate_collectibles, load_collection_pricing, AnyCollectible, collectible_name, collectible_set};
use crate::collections::collections_models::{
    CollectionValueCards, CollectionValueHistory, CurrencyValueHistory, DatedPrice, MoneyAmount, ValueCardEntry,
    ValueCardsQuery, ValueHistoryPoint, ValueHistoryQuery,
};
use crate::{ApiError, GathersState, storage_error};
use models::Card;

const DEFAULT_PAST_DAYS: u32 = 30;
const MAX_PAST_DAYS: u32 = 3650;

/// Non-finite totals (from absurd recorded prices) would serialize as null.
fn round2(v: f64) -> f64 {
    if v.is_finite() { (v * 100.0).round() / 100.0 } else { 0.0 }
}

/// Walks one card's recorded prices (oldest first) day by day, carrying
/// each finish's last price forward.
struct PriceTrack<'a> {
    prices: &'a [DailyPrice],
    next: usize,
    normal: Option<f64>,
    foil: Option<f64>,
}

impl<'a> PriceTrack<'a> {
    fn new(prices: &'a [DailyPrice]) -> Self {
        Self { prices, next: 0, normal: None, foil: None }
    }

    /// Takes in everything recorded up to and including `day`.
    fn advance_to(&mut self, day: NaiveDate) {
        while let Some(p) = self.prices.get(self.next).filter(|p| p.recorded_on <= day) {
            // Recordings only ever use "" and "foil" (see
            // `price_history::price_points`).
            match p.finish.as_str() {
                "" => self.normal = Some(p.price),
                "foil" => self.foil = Some(p.price),
                _ => {}
            }
            self.next += 1;
        }
    }

    /// Unit price of `finish` so far — same rule as `preferred_unit_prices`:
    /// "foil" uses the foil price, every other finish the normal one, each
    /// falling back to the other.
    fn unit(&self, finish: &str) -> Option<f64> {
        if finish == "foil" { self.foil.or(self.normal) } else { self.normal.or(self.foil) }
    }
}

/// Owned rows (quantity > 0) that have a current price.
fn owned_priced(pricing: &CollectionPricing) -> impl Iterator<Item = (&models::CollectionCard, &UnitPrices)> {
    pricing
        .cards
        .iter()
        .filter(|c| c.quantity > 0)
        .filter_map(|c| pricing.unit_prices.get(&c.uuid).map(|u| (c, u)))
}

/// Pure computation of the value series, given each priced card's history
/// (keyed by uuid, oldest first). Points before `since` are dropped, though
/// prices recorded before it still carry forward into it.
fn compute_value_history(
    owned: &[(&models::CollectionCard, &UnitPrices)],
    histories: &HashMap<String, Vec<DailyPrice>>,
    since: Option<NaiveDate>,
) -> Vec<CurrencyValueHistory> {
    let mut by_currency: HashMap<&str, Vec<&models::CollectionCard>> = HashMap::new();
    for (card, unit) in owned {
        by_currency.entry(unit.currency.as_str()).or_default().push(card);
    }

    let mut out: Vec<CurrencyValueHistory> = by_currency
        .into_iter()
        .map(|(currency, cards)| {
            let all_days: BTreeSet<NaiveDate> = cards
                .iter()
                .filter_map(|c| histories.get(&c.uuid))
                .flatten()
                .map(|p| p.recorded_on)
                .collect();
            let days: Vec<NaiveDate> = all_days.into_iter().filter(|d| since.is_none_or(|s| *d >= s)).collect();
            let mut values = vec![0.0; days.len()];
            let mut priced = vec![0usize; days.len()];
            for card in &cards {
                let Some(history) = histories.get(&card.uuid) else { continue };
                let mut track = PriceTrack::new(history);
                for (i, day) in days.iter().enumerate() {
                    track.advance_to(*day);
                    if let Some(price) = track.unit(&card.finish) {
                        values[i] += price * card.quantity as f64;
                        priced[i] += 1;
                    }
                }
            }
            CurrencyValueHistory {
                currency: currency.to_string(),
                entry_count: cards.len(),
                points: days
                    .into_iter()
                    .zip(values.into_iter().zip(priced))
                    .map(|(day, (value, priced_count))| ValueHistoryPoint { day, value: round2(value), priced_count })
                    .collect(),
            }
        })
        .collect();

    let latest = |c: &CurrencyValueHistory| c.points.last().map_or(0.0, |p| p.value);
    out.sort_by(|a, b| latest(b).total_cmp(&latest(a)).then_with(|| a.currency.cmp(&b.currency)));
    out
}

/// Each priced card's history from the retailer it's valued at, keyed by
/// uuid. Empty when price history is off or can't be read.
async fn load_histories(
    state: &GathersState,
    pricing: &CollectionPricing,
) -> Result<HashMap<String, Vec<DailyPrice>>, ApiError> {
    let storage = state.1.lock().await.storage.clone();
    let mut retailers_by_provider: HashMap<&str, HashMap<String, String>> = HashMap::new();
    for (card, _) in owned_priced(pricing) {
        if let Some(retailer) = pricing.retailers.get(&card.uuid) {
            retailers_by_provider
                .entry(card.provider.as_str())
                .or_default()
                .insert(card.uuid.clone(), retailer.clone());
        }
    }
    let mut out = HashMap::new();
    for (provider, retailers) in retailers_by_provider {
        let histories = storage
            .get_retailer_price_histories(provider, &retailers)
            .await
            .map_err(|e| storage_error("Failed to get price history", e))?;
        out.extend(histories);
    }
    Ok(out)
}

/// `/collection/cards/{id}/value_history`: see `CollectionValueHistory`.
pub(super) async fn value_history(
    State(state): State<GathersState>,
    Path(collection_id): Path<String>,
    Query(query): Query<ValueHistoryQuery>,
) -> Result<Json<CollectionValueHistory>, ApiError> {
    let enabled = state.1.lock().await.storage.price_history_enabled();
    let pricing = load_collection_pricing(&state, &collection_id).await?;
    let total_count = pricing.cards.iter().filter(|c| c.quantity > 0).count();
    let histories = if enabled { load_histories(&state, &pricing).await? } else { HashMap::new() };
    let owned: Vec<_> = owned_priced(&pricing).collect();
    let currencies = if enabled { compute_value_history(&owned, &histories, query.since) } else { vec![] };
    Ok(Json(CollectionValueHistory { enabled, total_count, currencies }))
}

/// Card details shown next to a value entry.
#[derive(Default)]
struct CardInfo {
    name: Option<String>,
    set_code: Option<String>,
    image: Option<String>,
    scryfall_id: Option<String>,
}

fn card_info(card: Option<&AnyCollectible>) -> CardInfo {
    let Some(card) = card else { return CardInfo::default() };
    let non_empty = |s: &str| (!s.is_empty()).then(|| s.to_string());
    let (image, scryfall_id) = match card {
        AnyCollectible::Known(Card::Magic(m)) => (None, non_empty(&m.card_identifiers.scryfall_id)),
        AnyCollectible::Known(Card::Riftbound(r)) => (non_empty(&r.image), None),
        AnyCollectible::Known(Card::Pokemon(p)) => (non_empty(&p.image), None),
        AnyCollectible::Plugin(p) => (p.image_url.clone(), None),
    };
    CardInfo { name: Some(collectible_name(card).to_string()), set_code: Some(collectible_set(card)), image, scryfall_id }
}

/// Pure computation of the per-entry valuations (without card details),
/// most valuable first.
fn compute_value_cards(
    pricing: &CollectionPricing,
    histories: &HashMap<String, Vec<DailyPrice>>,
    past_day: NaiveDate,
) -> Vec<ValueCardEntry> {
    let mut entries: Vec<ValueCardEntry> = owned_priced(pricing)
        .map(|(card, unit)| {
            let unit_price = if card.finish == "foil" { unit.foil } else { unit.normal };

            // Cost basis, same as `compute_value_breakdown`: only `paid` of
            // the `recorded` copies are still owned, so each currency's cost
            // is scaled down proportionally.
            let summaries = pricing
                .purchase_totals
                .get(&(card.uuid.clone(), card.finish.clone()))
                .map(Vec::as_slice)
                .unwrap_or_default();
            let recorded: i32 = summaries.iter().map(|s| s.quantity).fold(0, i32::saturating_add);
            let paid = recorded.min(card.quantity);
            let owned_share = if recorded > 0 { paid as f64 / recorded as f64 } else { 0.0 };
            let mut cost: Vec<MoneyAmount> = Vec::new();
            for s in summaries {
                match cost.iter_mut().find(|m| m.currency == s.currency) {
                    Some(m) => m.value += s.total_paid * owned_share,
                    None => cost.push(MoneyAmount { currency: s.currency.clone(), value: s.total_paid * owned_share }),
                }
            }
            cost.retain(|m| m.value > 0.0);
            for m in &mut cost {
                m.value = round2(m.value);
            }
            let profit = (paid > 0 && cost.iter().all(|m| m.currency == unit.currency))
                .then(|| round2(unit_price * paid as f64 - cost.iter().map(|m| m.value).sum::<f64>()));

            let mut first_price = None;
            let mut past_price = None;
            if let Some(history) = histories.get(&card.uuid) {
                let mut track = PriceTrack::new(history);
                let days: BTreeSet<NaiveDate> = history.iter().map(|p| p.recorded_on).collect();
                for day in days {
                    track.advance_to(day);
                    let Some(price) = track.unit(&card.finish) else { continue };
                    first_price.get_or_insert(DatedPrice { day, price });
                    if day <= past_day {
                        past_price = Some(DatedPrice { day, price });
                    } else {
                        break;
                    }
                }
            }

            ValueCardEntry {
                card_uuid: card.uuid.clone(),
                provider: card.provider.clone(),
                finish: card.finish.clone(),
                name: None,
                set_code: None,
                image: None,
                scryfall_id: None,
                quantity: card.quantity,
                currency: unit.currency.clone(),
                unit_price,
                total_value: round2(unit_price * card.quantity as f64),
                cost_quantity: paid,
                cost,
                profit,
                first_price,
                past_price,
            }
        })
        // Same as the breakdown: a zero price isn't a price.
        .filter(|e| e.total_value > 0.0)
        .collect();
    entries.sort_by(|a, b| {
        b.total_value
            .total_cmp(&a.total_value)
            .then_with(|| a.card_uuid.cmp(&b.card_uuid))
            .then_with(|| a.finish.cmp(&b.finish))
    });
    entries
}

/// `/collection/cards/{id}/value_cards`: see `CollectionValueCards`.
pub(super) async fn value_cards(
    State(state): State<GathersState>,
    Path(collection_id): Path<String>,
    Query(query): Query<ValueCardsQuery>,
) -> Result<Json<CollectionValueCards>, ApiError> {
    let history_enabled = state.1.lock().await.storage.price_history_enabled();
    let pricing = load_collection_pricing(&state, &collection_id).await?;
    let histories = if history_enabled { load_histories(&state, &pricing).await? } else { HashMap::new() };
    let days = query.days.unwrap_or(DEFAULT_PAST_DAYS).clamp(1, MAX_PAST_DAYS);
    let past_day = chrono::Utc::now().date_naive() - chrono::Days::new(days.into());

    let mut entries = compute_value_cards(&pricing, &histories, past_day);
    let owned = pricing.cards.iter().filter(|c| c.quantity > 0).count();
    let unpriced_count = owned - entries.len();

    let mut uuids_by_provider: HashMap<String, Vec<String>> = HashMap::new();
    for e in &entries {
        uuids_by_provider.entry(e.provider.clone()).or_default().push(e.card_uuid.clone());
    }
    for ids in uuids_by_provider.values_mut() {
        ids.sort();
        ids.dedup();
    }
    let cards = hydrate_collectibles(&state, &uuids_by_provider).await;
    for e in &mut entries {
        let info = card_info(cards.get(&e.card_uuid));
        e.name = info.name;
        e.set_code = info.set_code;
        e.image = info.image;
        e.scryfall_id = info.scryfall_id;
    }

    Ok(Json(CollectionValueCards { history_enabled, past_day, unpriced_count, entries }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(uuid: &str, finish: &str, quantity: i32) -> models::CollectionCard {
        models::CollectionCard {
            uuid: uuid.to_string(),
            finish: finish.to_string(),
            quantity,
            want_quantity: 0,
            time_added: "2024-01-01T00:00:00Z".to_string(),
            collection: "c1".to_string(),
            provider: "mtg".to_string(),
        }
    }

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn daily(finish: &str, price: f64, on: &str) -> DailyPrice {
        DailyPrice { finish: finish.to_string(), price, recorded_on: day(on) }
    }

    fn unit(normal: f64, foil: f64, currency: &str) -> UnitPrices {
        UnitPrices { normal, foil, currency: currency.to_string() }
    }

    fn pricing(cards: Vec<models::CollectionCard>, units: Vec<(&str, UnitPrices)>) -> CollectionPricing {
        CollectionPricing {
            cards,
            unit_prices: units.into_iter().map(|(u, p)| (u.to_string(), p)).collect(),
            retailers: HashMap::new(),
            purchase_totals: HashMap::new(),
        }
    }

    fn history(p: &CollectionPricing, h: &HashMap<String, Vec<DailyPrice>>, since: Option<&str>) -> Vec<CurrencyValueHistory> {
        let owned: Vec<_> = owned_priced(p).collect();
        compute_value_history(&owned, h, since.map(day))
    }

    fn summary(points: &[ValueHistoryPoint]) -> Vec<(String, f64, usize)> {
        points.iter().map(|p| (p.day.to_string(), p.value, p.priced_count)).collect()
    }

    #[test]
    fn sums_quantity_times_price_and_carries_prices_forward() {
        let p = pricing(
            vec![card("a", "", 2), card("b", "", 1)],
            vec![("a", unit(1.0, 1.0, "USD")), ("b", unit(1.0, 1.0, "USD"))],
        );
        let h = HashMap::from([
            ("a".to_string(), vec![daily("", 1.0, "2026-01-01"), daily("", 1.5, "2026-01-03")]),
            // b's history starts later: it adds nothing before then.
            ("b".to_string(), vec![daily("", 10.0, "2026-01-02")]),
        ]);

        let series = history(&p, &h, None);

        assert_eq!(series.len(), 1);
        assert_eq!(series[0].entry_count, 2);
        assert_eq!(
            summary(&series[0].points),
            vec![
                ("2026-01-01".to_string(), 2.0, 1),
                ("2026-01-02".to_string(), 12.0, 2),
                ("2026-01-03".to_string(), 13.0, 2),
            ]
        );
    }

    #[test]
    fn finishes_use_their_own_price_falling_back_to_the_other() {
        let p = pricing(
            vec![card("a", "", 1), card("a", "foil", 1), card("b", "foil", 1), card("c", "etched", 1)],
            vec![("a", unit(1.0, 5.0, "USD")), ("b", unit(2.0, 2.0, "USD")), ("c", unit(3.0, 3.0, "USD"))],
        );
        let h = HashMap::from([
            ("a".to_string(), vec![daily("", 1.0, "2026-01-01"), daily("foil", 5.0, "2026-01-01")]),
            ("b".to_string(), vec![daily("", 2.0, "2026-01-01")]),
            ("c".to_string(), vec![daily("foil", 3.0, "2026-01-01")]),
        ]);

        let series = history(&p, &h, None);

        assert_eq!(summary(&series[0].points), vec![("2026-01-01".to_string(), 11.0, 4)]);
    }

    #[test]
    fn excludes_wanted_only_and_unpriced_rows_and_splits_currencies() {
        let p = pricing(
            vec![card("a", "", 1), card("w", "", 0), card("u", "", 3), card("e", "", 1)],
            vec![("a", unit(1.0, 1.0, "USD")), ("w", unit(9.0, 9.0, "USD")), ("e", unit(4.0, 4.0, "EUR"))],
        );
        let h = HashMap::from([
            ("a".to_string(), vec![daily("", 1.0, "2026-01-01")]),
            ("w".to_string(), vec![daily("", 9.0, "2026-01-01")]),
            ("u".to_string(), vec![daily("", 9.0, "2026-01-01")]),
            ("e".to_string(), vec![daily("", 4.0, "2026-01-02")]),
        ]);

        let series = history(&p, &h, None);

        // Largest latest value first.
        assert_eq!(series.iter().map(|s| s.currency.as_str()).collect::<Vec<_>>(), vec!["EUR", "USD"]);
        assert_eq!(summary(&series[0].points), vec![("2026-01-02".to_string(), 4.0, 1)]);
        assert_eq!(summary(&series[1].points), vec![("2026-01-01".to_string(), 1.0, 1)]);
    }

    #[test]
    fn since_drops_earlier_days_but_keeps_their_prices() {
        let p = pricing(vec![card("a", "", 1), card("b", "", 1)], vec![("a", unit(1.0, 1.0, "USD")), ("b", unit(1.0, 1.0, "USD"))]);
        let h = HashMap::from([
            ("a".to_string(), vec![daily("", 1.0, "2026-01-01")]),
            ("b".to_string(), vec![daily("", 2.0, "2026-01-01"), daily("", 3.0, "2026-01-05")]),
        ]);

        let series = history(&p, &h, Some("2026-01-02"));

        assert_eq!(summary(&series[0].points), vec![("2026-01-05".to_string(), 4.0, 2)]);
    }

    #[test]
    fn value_cards_rank_by_value_with_cost_and_movement() {
        let mut p = pricing(
            vec![card("a", "", 2), card("b", "foil", 1), card("c", "", 1), card("w", "", 0)],
            vec![("a", unit(10.0, 10.0, "USD")), ("b", unit(1.0, 30.0, "USD")), ("c", unit(5.0, 5.0, "USD"))],
        );
        let summary = |currency: &str, total_paid: f64, quantity: i32| persistence::PurchaseSummary {
            currency: currency.to_string(),
            total_paid,
            quantity,
        };
        p.purchase_totals = HashMap::from([
            // 3 bought for $6 in all, only 2 still owned.
            (("a".to_string(), String::new()), vec![summary("USD", 6.0, 3)]),
            (("b".to_string(), "foil".to_string()), vec![summary("EUR", 12.0, 1)]),
        ]);
        let h = HashMap::from([(
            "b".to_string(),
            vec![daily("foil", 20.0, "2026-01-01"), daily("foil", 25.0, "2026-01-10"), daily("foil", 30.0, "2026-01-20")],
        )]);

        let entries = compute_value_cards(&p, &h, day("2026-01-15"));

        let ids: Vec<_> = entries.iter().map(|e| e.card_uuid.as_str()).collect();
        assert_eq!(ids, vec!["b", "a", "c"]);
        let (b, a, c) = (&entries[0], &entries[1], &entries[2]);
        assert_eq!((b.unit_price, b.total_value), (30.0, 30.0));
        // Bought in another currency: no server-side profit.
        assert_eq!(b.cost, vec![MoneyAmount { currency: "EUR".to_string(), value: 12.0 }]);
        assert_eq!(b.profit, None);
        assert_eq!(b.first_price, Some(DatedPrice { day: day("2026-01-01"), price: 20.0 }));
        assert_eq!(b.past_price, Some(DatedPrice { day: day("2026-01-10"), price: 25.0 }));
        assert_eq!(a.cost_quantity, 2);
        assert_eq!(a.cost, vec![MoneyAmount { currency: "USD".to_string(), value: 4.0 }]);
        assert_eq!(a.profit, Some(16.0));
        assert_eq!((c.cost_quantity, c.profit, c.past_price.clone()), (0, None, None));
    }

    #[test]
    fn value_cards_past_price_missing_when_history_starts_later() {
        let p = pricing(vec![card("a", "", 1)], vec![("a", unit(2.0, 2.0, "USD"))]);
        let h = HashMap::from([("a".to_string(), vec![daily("", 1.0, "2026-02-01")])]);

        let entries = compute_value_cards(&p, &h, day("2026-01-15"));

        assert_eq!(entries[0].past_price, None);
        assert_eq!(entries[0].first_price, Some(DatedPrice { day: day("2026-02-01"), price: 1.0 }));
    }
}
