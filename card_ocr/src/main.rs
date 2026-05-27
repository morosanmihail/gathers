use card_ocr::identify_card;
use models::{Card, CardTrait};
use retrieval::MagicSQLiteRetrievalSystem;
use std::{env, path::PathBuf};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: card_ocr <image_path> [db_path]");
        #[cfg(feature = "onnx-seg")]
        eprintln!("  YOLO_MODEL_PATH  path to YOLOv8 ONNX model for card segmentation");
        std::process::exit(1);
    }

    let image_path = PathBuf::from(&args[1]);
    let db_path = args.get(2).cloned();
    let retrieval = MagicSQLiteRetrievalSystem::new(db_path, None)?;

    match identify_card(&image_path, &retrieval).await? {
        Some(card) => {
            let name = match &card {
                Card::Magic(c) => c.name.as_str(),
                Card::Riftbound(c) => c.name.as_str(),
                Card::Pokemon(c) => c.name.as_str(),
            };
            println!(
                "Identified: {} ({} #{})",
                name,
                card.get_set(),
                card.get_collector_number()
            );
        }
        None => println!("Could not identify card."),
    }

    Ok(())
}
