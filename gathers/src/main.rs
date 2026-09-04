use clap::{Parser, ValueEnum};
use models::{Card, CardColour, Rarity, pokemon::EnergyType, riftbound::CardDomain};
use retrieval::{RetrievalSystem, RetrievalSystemTrait};
use std::path::PathBuf;

#[derive(Copy, Clone, ValueEnum, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Systems {
    Scryfall,
    Sql,
    RiftboundSql,
    Pokemon,
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[clap(short, long, default_value = "scryfall")]
    system: Systems,

    #[clap(short, long)]
    name: Option<String>,

    #[clap(short, long, default_value = "10")]
    limit: usize,

    #[clap(short, long, default_value = "0")]
    offset: usize,

    #[clap(short, long)]
    color: Vec<String>,

    #[clap(long)]
    set: Option<String>,

    #[clap(long)]
    collector_number: Option<String>,

    #[clap(short, long)]
    artist: Option<String>,

    #[clap(short, long)]
    text: Option<String>,

    #[clap(short, long)]
    rarity: Option<String>,

    #[clap(long)]
    subtype: Option<Vec<String>>,

    #[clap(long)]
    supertype: Option<String>,

    #[clap(long)]
    types: Option<Vec<String>>,

    #[clap(long)]
    domain: Vec<String>,

    #[clap(long)]
    energy: Vec<String>,

    #[clap(short, long)]
    download: bool,

    #[clap(long)]
    price: Option<String>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();

    let retrieval_db_path: Option<String> = match args.system {
        Systems::Scryfall => None,
        Systems::Sql => {
            let raw = std::env::var("RETRIEVAL_DB_PATH")
                .unwrap_or_else(|_| "AllPrintings.db".to_string());
            Some(resolve_db_path(&raw, "AllPrintings.db"))
        }
        Systems::RiftboundSql => {
            let raw =
                std::env::var("RETRIEVAL_DB_PATH").unwrap_or_else(|_| "riftbound.db".to_string());
            Some(resolve_db_path(&raw, "riftbound.db"))
        }
        Systems::Pokemon => {
            let raw =
                std::env::var("RETRIEVAL_DB_PATH").unwrap_or_else(|_| "pokemon.db".to_string());
            Some(resolve_db_path(&raw, "pokemon.db"))
        }
    };

    if let Some(ref path) = retrieval_db_path {
        let db_exists = PathBuf::from(path).exists();

        if args.download || !db_exists {
            if !db_exists && !args.download {
                println!("Database not found at: {path}");
                println!("Would you like to download/update it? (y/n)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if input.trim().to_lowercase() != "y" {
                    println!("Run with --download to fetch the database.");
                    return Ok(());
                }
            }

            match args.system {
                Systems::Sql => {
                    retrieval::download_mtg_db(path, None).await?;
                }
                Systems::RiftboundSql => {
                    let tmp = retrieval::RiftboundSQLiteRetrievalSystem::new(Some(path.clone()))?;
                    RetrievalSystem::RiftboundSQLiteRetrievalSystem(tmp)
                        .update_backend()
                        .await?;
                }
                Systems::Scryfall => unreachable!(),
                Systems::Pokemon => {
                    let tmp = retrieval::PokemonSQLiteRetrievalSystem::new(Some(path.clone()), None)?;
                    RetrievalSystem::PokemonSQLiteRetrievalSystem(tmp)
                        .update_backend()
                        .await?;
                }
            }

            if args.download {
                return Ok(());
            }
        }
    } else if args.download {
        println!("Scryfall does not use a local database.");
        return Ok(());
    }

    let retrieval: RetrievalSystem = match args.system {
        Systems::Scryfall => {
            RetrievalSystem::ScryfallRetrievalSystem(retrieval::ScryfallRetrievalSystem {})
        }
        Systems::Sql => RetrievalSystem::MagicSQLiteRetrievalSystem(
            retrieval::MagicSQLiteRetrievalSystem::new(retrieval_db_path, args.price.clone())?,
        ),
        Systems::RiftboundSql => RetrievalSystem::RiftboundSQLiteRetrievalSystem(
            retrieval::RiftboundSQLiteRetrievalSystem::new(retrieval_db_path)?,
        ),
        Systems::Pokemon => RetrievalSystem::PokemonSQLiteRetrievalSystem(
            retrieval::PokemonSQLiteRetrievalSystem::new(retrieval_db_path, None)?,
        ),
    };

    let color_identities: Option<Vec<CardColour>> = if args.color.is_empty() {
        None
    } else {
        Some(args.color.iter().map(|s| CardColour::from(s.as_str())).collect())
    };

    let domains: Option<Vec<CardDomain>> = if args.domain.is_empty() {
        None
    } else {
        Some(args.domain.into_iter().map(CardDomain::from).collect())
    };

    let energy_types: Option<Vec<EnergyType>> = if args.energy.is_empty() {
        None
    } else {
        Some(args.energy.into_iter().map(EnergyType::from).collect())
    };

    let rarity: Option<Rarity> = args.rarity.map(|r| r.into());

    let cards = retrieval
        .search_cards(
            models::filters::CardSearchFilters {
                name: args.name,
                color_identities,
                set_code: args.set,
                collector_number: args.collector_number,
                artist: args.artist,
                text: args.text,
                rarity,
                subtypes: args.subtype,
                supertypes: args.supertype,
                types: args.types,
                domains,
                energy_types,
                sort_by: None,
                sort_order: None,
                ..Default::default()
            },
            Some(args.offset),
            Some(args.limit),
        )
        .await?;

    if cards.is_empty() {
        println!("No cards found matching the criteria.");
        return Ok(());
    }

    println!("Found {} card(s):\n", cards.len());

    match args.system {
        Systems::Scryfall | Systems::Sql => {
            let magic_cards: Vec<_> = cards
                .into_iter()
                .map(|card| {
                    if let Card::Magic(card) = card {
                        card
                    } else {
                        panic!("Not a Magic card")
                    }
                })
                .collect();

            let prices = if args.price.is_some() {
                let uuids = magic_cards.iter().map(|c| c.id.clone()).collect();
                retrieval.get_bulk_card_prices(uuids).await?
            } else {
                std::collections::HashMap::new()
            };

            if args.price.is_some() {
                println!(
                    "{:<30} {:<5} {:<10} {:<7} {:<25} {:<10} {:<15} {:<15} {:<15} {:<12} {:<12}",
                    "Name", "Set", "Rarity", "Colors", "Artist", "Number", "Subtype", "Supertype", "Types", "Normal", "Foil"
                );
                println!("{}", "-".repeat(165));
            } else {
                println!(
                    "{:<30} {:<5} {:<10} {:<7} {:<25} {:<10} {:<15} {:<15} {:<15}",
                    "Name", "Set", "Rarity", "Colors", "Artist", "Number", "Subtype", "Supertype", "Types"
                );
                println!("{}", "-".repeat(140));
            }

            for card in magic_cards {
                let color_str: String = card
                    .color_identity
                    .iter()
                    .map(|c| format!("{}", c))
                    .collect::<Vec<_>>()
                    .join("");
                if args.price.is_some() {
                    let fmt_price = |f: fn(&models::RetailerPrices) -> Option<f64>| {
                        prices
                            .get(&card.id)
                            .and_then(|p| {
                                p.paper.values().filter_map(f).reduce(f64::min).map(|v| format!("${:.2}", v))
                            })
                            .unwrap_or_else(|| "N/A".to_string())
                    };
                    let normal = fmt_price(|r| r.normal);
                    let foil = fmt_price(|r| r.foil);
                    println!(
                        "{:<30} {:<5} {:<10} {:<7} {:<25} {:<10} {:<15} {:<15} {:<15} {:<12} {:<12}",
                        card.name,
                        card.set_code,
                        card.rarity.to_string().to_lowercase(),
                        color_str,
                        card.artist,
                        card.collector_number,
                        card.subtypes.join(","),
                        card.supertypes.join(","),
                        card.types.join(","),
                        normal,
                        foil,
                    );
                } else {
                    println!(
                        "{:<30} {:<5} {:<10} {:<7} {:<25} {:<10} {:<15} {:<15} {:<15}",
                        card.name,
                        card.set_code,
                        card.rarity.to_string().to_lowercase(),
                        color_str,
                        card.artist,
                        card.collector_number,
                        card.subtypes.join(","),
                        card.supertypes.join(","),
                        card.types.join(","),
                    );
                }
            }
        }
        Systems::RiftboundSql => {
            println!(
                "{:<30} {:<5} {:<10} {:<30} {:<25} {:<10}",
                "Name", "Set", "Rarity", "Domains", "Artists", "Number"
            );
            println!("{}", "-".repeat(115));
            for card in cards {
                let card = if let Card::Riftbound(card) = card {
                    card
                } else {
                    panic!("Not a Riftbound card")
                };
                let domain_str = card
                    .domains
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let artist_str = card.artists.join(",");
                println!(
                    "{:<30} {:<5} {:<10} {:<30} {:<25} {:<10}",
                    card.name,
                    card.set_code,
                    card.rarity.to_string().to_lowercase(),
                    domain_str,
                    artist_str,
                    card.collector_number,
                );
            }
        }
        Systems::Pokemon => {
            println!(
                "{:<20} {:<35} {:<18} {:<10}",
                "Name", "Set", "Rarity", "Number"
            );
            println!("{}", "-".repeat(85));
            for card in cards {
                let card = if let Card::Pokemon(card) = card {
                    card
                } else {
                    panic!("Not a Pokemon card")
                };
                println!(
                    "{:<20} {:<35} {:<18} {:<10}",
                    card.name,
                    card.set_code,
                    card.rarity.to_string().to_lowercase(),
                    card.collector_number,
                );
            }
        }
    }

    Ok(())
}

fn resolve_db_path(raw: &str, filename: &str) -> String {
    if raw.starts_with('.') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let dir = PathBuf::from(home).join(".local/share/gathers");
        std::fs::create_dir_all(&dir).ok();
        dir.join(filename).to_string_lossy().into_owned()
    } else {
        raw.to_string()
    }
}
