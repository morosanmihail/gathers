mod extraction;
pub use extraction::{extract_card, extract_card_debug, ExtractionStatus};

pub mod segmentation;
pub use segmentation::{HoughSegmenter, Segmenter};
#[cfg(feature = "opencv-seg")]
pub use segmentation::OpenCvSegmenter;
#[cfg(feature = "onnx-seg")]
pub use segmentation::OnnxSegmenter;

use eyre::{Result, WrapErr};
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use leptess::LepTess;
use models::{Card, filters::CardSearchFilters};
use retrieval::RetrievalSystemTrait;
use std::{io::Cursor, path::Path};

/// Signals extracted from the bottom strip.
struct BottomOcr {
    set_code: Option<String>,
    collector_number: Option<String>,
    /// Confidence level of collector_number:
    /// 1 = 4-digit zero-padded (high), 2 = adjacent pair (medium), 3 = single token (low).
    number_confidence: u8,
}

/// Identify a Magic card from an image file.
///
/// Thin wrapper around [`identify_card_image`] that opens the file first.
pub async fn identify_card<R: RetrievalSystemTrait>(
    image_path: &Path,
    retrieval: &R,
) -> Result<Option<Card>> {
    let img = image::open(image_path)
        .wrap_err_with(|| format!("Failed to open image: {}", image_path.display()))?;
    let segmented = try_segment(&img);
    identify_card_image(&segmented, retrieval).await
}

/// Try each available segmenter in order (OpenCV → ONNX).
/// Returns the first successful extraction, or a clone of the original if all fail.
fn try_segment(img: &DynamicImage) -> DynamicImage {
    #[cfg(feature = "opencv-seg")]
    {
        if let Some(result) = segmentation::OpenCvSegmenter.extract(img) {
            tracing::debug!("Segmented with opencv-contour");
            return result;
        }
    }
    #[cfg(feature = "onnx-seg")]
    {
        if let Ok(seg) = segmentation::OnnxSegmenter::from_env() {
            if let Some(result) = seg.extract(img) {
                tracing::debug!("Segmented with onnx-yolo");
                return result;
            }
        }
    }
    tracing::debug!("No segmenter succeeded, using raw image");
    img.clone()
}

/// Identify a Magic card from an already-decoded image.
///
/// Combines bottom-strip signals (set code, collector number) with top-strip
/// signals (card name candidates) and tries matches in confidence order:
///   P1 – set + number          (verified by name if confidence < 1 and names available)
///   P2 – set + name            (handles OCR-mangled numbers)
///   P3 – name + number         (handles missing set code with known number)
///   P4 – name only             (last resort; may return a different printing)
pub async fn identify_card_image<R: RetrievalSystemTrait>(
    img: &DynamicImage,
    retrieval: &R,
) -> Result<Option<Card>> {
    let bottom = read_bottom_strip(img)?;
    let names = read_top_strip(img)?;

    // P1: set + number — most precise.
    // For high-confidence numbers (4-digit padded), trust immediately.
    // For lower confidence, check whether the name agrees; save result as fallback
    // in case name-based strategies below can find a better match.
    let mut p1_fallback: Option<Card> = None;
    if let (Some(set), Some(num)) = (&bottom.set_code, &bottom.collector_number) {
        tracing::debug!("Bottom strip OCR: set={set} number={num} confidence={}", bottom.number_confidence);
        let filters = CardSearchFilters::new()
            .with_set_code(set)
            .with_collector_number(num);
        let cards = retrieval.search_cards(filters, None, Some(1)).await?;
        if let Some(card) = cards.into_iter().next() {
            let name_ok = bottom.number_confidence == 1
                || names.is_empty()
                || names.iter().any(|n| card_name_matches(&card, n));
            if name_ok {
                return Ok(Some(card));
            }
            // Name disagreement: number may be OCR-mangled. Save as fallback and
            // try name-based strategies; return this if nothing better is found.
            tracing::debug!("P1 name mismatch for {set}/{num} — trying alternatives");
            p1_fallback = Some(card);
        }
    }

    // P2: set + name — recovers when OCR mangled the collector number.
    if let Some(ref set) = bottom.set_code {
        for name in &names {
            tracing::debug!("P2 trying set={set} name={name:?}");
            let filters = CardSearchFilters::new().with_set_code(set).with_name(name);
            let cards = retrieval.search_cards(filters, None, Some(1)).await?;
            if let Some(card) = cards.into_iter().next() {
                return Ok(Some(card));
            }
        }
    }

    // P3: name + number — recovers when set code is absent but number is reliable.
    if let Some(ref num) = bottom.collector_number {
        for name in &names {
            tracing::debug!("P3 trying name={name:?} number={num}");
            let filters = CardSearchFilters::new().with_name(name).with_collector_number(num);
            let cards = retrieval.search_cards(filters, None, Some(1)).await?;
            if let Some(card) = cards.into_iter().next() {
                return Ok(Some(card));
            }
        }
    }

    // P4: name only — last resort; may return a different printing.
    for name in &names {
        tracing::debug!("P4 trying name={name:?}");
        let filters = CardSearchFilters::new().with_name(name);
        let cards = retrieval.search_cards(filters, None, Some(1)).await?;
        if let Some(card) = cards.into_iter().next() {
            return Ok(Some(card));
        }
    }

    // If P1 found a card but name disagreed and no alternative was found, trust P1.
    Ok(p1_fallback)
}

/// Returns true if an OCR-extracted name plausibly matches the card's stored name.
fn card_name_matches(card: &Card, ocr_name: &str) -> bool {
    let card_name = match card {
        Card::Magic(c) => c.name.to_lowercase(),
        Card::Riftbound(c) => c.name.to_lowercase(),
        Card::Pokemon(c) => c.name.to_lowercase(),
    };
    let ocr_lower = ocr_name.to_lowercase();
    // Exact match or substring in either direction (handles double-faced card names
    // like "Tarrian's Journal // The Tomb of Aclazotz" vs OCR "Tarrian's Journal").
    card_name == ocr_lower
        || card_name.contains(&ocr_lower)
        || ocr_lower.contains(&card_name)
}

fn read_bottom_strip(img: &DynamicImage) -> Result<BottomOcr> {
    let (w, h) = img.dimensions();
    let strip_h = (h as f32 * 0.15) as u32;
    let y = h - strip_h;
    let strip = img.crop_imm(0, y, w, strip_h);

    // Bottom strip has white text on black — invert so Tesseract sees dark-on-light.
    let mut gray = strip.to_luma8();
    image::imageops::invert(&mut gray);
    let prepped = DynamicImage::ImageLuma8(gray);
    let scaled = prepped.resize(w * 3, strip_h * 3, FilterType::Lanczos3);

    // No character whitelist: special separators like •, ★, / vary by printing era.
    // Instead we normalise after OCR (uppercase + alphanumeric only).
    let raw = run_ocr(&scaled)?;
    let text = normalize_bottom(raw.as_str());
    tracing::debug!("Bottom strip OCR text: {text:?}");
    Ok(parse_bottom(&text))
}

fn read_top_strip(img: &DynamicImage) -> Result<Vec<String>> {
    let (w, h) = img.dimensions();
    let strip_h = (h as f32 * 0.12) as u32;
    let strip = img.crop_imm(0, 0, w, strip_h);
    let scaled = strip.resize(w * 3, strip_h * 3, FilterType::Lanczos3);

    let raw = run_ocr(&scaled)?;
    // Normalise typographic apostrophes so "Executioner's" etc. survive the name split.
    let text = raw.replace('\u{2019}', "'").replace('\u{2018}', "'");
    tracing::debug!("Top strip OCR text: {text:?}");
    Ok(parse_top_candidates(&text))
}

fn run_ocr(img: &DynamicImage) -> Result<String> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .wrap_err("Failed to encode image to PNG")?;
    let bytes = buf.into_inner();

    let mut lt = LepTess::new(None, "eng").wrap_err("Failed to initialise Tesseract")?;
    lt.set_image_from_mem(&bytes)
        .wrap_err("Failed to load image into Tesseract")?;
    lt.get_utf8_text().wrap_err("Tesseract OCR failed")
}

/// Convert raw OCR output from the bottom strip to a clean token stream.
/// Uppercases everything and replaces any non-alphanumeric character with a
/// space so that separators like •, ★, /, ✦ all become word boundaries.
fn normalize_bottom(text: &str) -> String {
    text.to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect()
}

// ISO 639-1 codes printed on MTG cards after the set code.
const LANG_CODES: &[&str] = &[
    "EN", "DE", "FR", "IT", "ES", "PT", "JP", "KO", "RU", "CS", "CT", "PH",
];

fn parse_bottom(text: &str) -> BottomOcr {
    // After normalisation the bottom strip looks like one of:
    //   Modern 4-digit:   "C 0303  MKC EN PIOTR DURA  2024 WIZARDS"
    //   NNN/NNN fraction: "186 274 U  BFZ EN KIERAN YANNER  2015 WIZARDS"
    //   Old frame:        no SET EN pair — set_code is None
    //
    // Set code: 2-5 alphanumeric token (at least one uppercase letter) before a known lang code.
    // Collector number: see find_collector_number_with_confidence.

    let tokens: Vec<&str> = text.split_whitespace().collect();

    let set_code = tokens.windows(2).find_map(|w| {
        let (a, b) = (w[0], w[1]);
        // Set codes are 2-5 alphanumeric chars (e.g. "ECL", "C19", "40K") with
        // at least one uppercase letter, followed by a known language code.
        if a.len() >= 2
            && a.len() <= 5
            && a.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && a.chars().any(|c| c.is_ascii_uppercase())
            && LANG_CODES.contains(&b)
        {
            Some(a.to_string())
        } else {
            None
        }
    });

    let (collector_number, number_confidence) = find_collector_number_with_confidence(&tokens);

    BottomOcr { set_code, collector_number, number_confidence }
}

fn find_collector_number_with_confidence(tokens: &[&str]) -> (Option<String>, u8) {
    // Strategy 1 (confidence 1): exactly 4-digit zero-padded token (modern frames, e.g. "0303").
    // Exclude copyright years.
    let padded4 = tokens.iter().find_map(|t| {
        if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
            let n: u32 = t.parse().unwrap_or(0);
            if !(1900..=2100).contains(&n) {
                return Some(strip_leading_zeros(t));
            }
        }
        None
    });
    if padded4.is_some() {
        return (padded4, 1);
    }

    // Strategy 2 (confidence 2): adjacent numeric pair (a, b) from "NNN/NNN" collector fractions
    // (pre-2022 frames). The slash is normalised to a space, so we see two digit-only tokens in
    // sequence where a < b (a = card number, b = set size).
    let pair = tokens.windows(2).find_map(|w| {
        let (a, b) = (w[0], w[1]);
        if a.len() >= 2
            && a.chars().all(|c| c.is_ascii_digit())
            && b.chars().all(|c| c.is_ascii_digit())
        {
            let na: u32 = a.parse().unwrap_or(0);
            let nb: u32 = b.parse().unwrap_or(0);
            if na > 0
                && na < nb
                && !(1900..=2100).contains(&na)
                && !(1900..=2100).contains(&nb)
            {
                return Some(strip_leading_zeros(a));
            }
        }
        None
    });
    if pair.is_some() {
        return (pair, 2);
    }

    // Strategy 3 (confidence 3): single numeric token with the most digits, excluding years.
    // Handles cases where OCR missed the set-size denominator (e.g. "135 2021" instead of
    // "135 311 2021"), leaving only the card number without a pair.
    let single = tokens.iter()
        .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()))
        .filter(|t| {
            let n: u32 = t.parse().unwrap_or(0);
            n > 0 && !(1900..=2100).contains(&n)
        })
        .max_by_key(|t| t.len())
        .map(|t| strip_leading_zeros(t));
    (single, 3)
}

fn parse_top_candidates(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| {
            let non_ws = l.chars().filter(|c| !c.is_whitespace()).count();
            if non_ws == 0 { return false; }
            let alpha = l.chars().filter(|c| c.is_alphabetic()).count();
            alpha * 100 / non_ws >= 65
        })
        .filter_map(extract_name_from_line)
        .collect()
}

fn extract_name_from_line(line: &str) -> Option<String> {
    let parts: Vec<&str> = line
        .split(|c: char| {
            !c.is_alphabetic() && c != ',' && c != '\'' && c != '-' && c != ' '
        })
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let best = parts
        .iter()
        .max_by_key(|p| p.chars().filter(|c| c.is_alphabetic()).count())?;

    let name = clean_name(best);
    if name.chars().filter(|c| c.is_alphabetic()).count() >= 5 {
        Some(name)
    } else {
        None
    }
}

fn clean_name(s: &str) -> String {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let is_real = |t: &&str| t.chars().filter(|c| c.is_alphabetic()).count() >= 2;
    let first = tokens.iter().position(is_real).unwrap_or(0);
    let last = tokens.iter().rposition(is_real).map(|i| i + 1).unwrap_or(tokens.len());
    tokens[first..last]
        .join(" ")
        .trim_matches(|c: char| !c.is_alphabetic())
        .to_string()
}

fn strip_leading_zeros(s: &str) -> String {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    let suffix: String = s.chars().skip_while(|c| c.is_ascii_digit()).collect();
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        format!("0{suffix}")
    } else {
        format!("{trimmed}{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_leading_zeros() {
        assert_eq!(strip_leading_zeros("0002"), "2");
        assert_eq!(strip_leading_zeros("0031"), "31");
        assert_eq!(strip_leading_zeros("0213"), "213");
        assert_eq!(strip_leading_zeros("0000"), "0");
        assert_eq!(strip_leading_zeros("002a"), "2a");
    }

    #[test]
    fn test_normalize_bottom_special_chars() {
        // • ★ / ✦ all become spaces; lowercase becomes uppercase.
        assert_eq!(normalize_bottom("BLC★EN"), "BLC EN");
        assert_eq!(normalize_bottom("MKC•EN"), "MKC EN");
        assert_eq!(normalize_bottom("186/274"), "186 274");
        assert_eq!(normalize_bottom("058/285"), "058 285");
    }

    #[test]
    fn test_parse_bottom_4digit_modern() {
        let text = "C 0002 ECL EN NILS HAMM 2026 WZ C";
        let ocr = parse_bottom(text);
        assert_eq!(ocr.set_code.as_deref(), Some("ECL"));
        assert_eq!(ocr.collector_number.as_deref(), Some("2"));
        assert_eq!(ocr.number_confidence, 1);
    }

    #[test]
    fn test_parse_bottom_4digit_large() {
        let text = "R 0303 MKC EN PIOTR DURA 2024 WIZARDS";
        let ocr = parse_bottom(text);
        assert_eq!(ocr.set_code.as_deref(), Some("MKC"));
        assert_eq!(ocr.collector_number.as_deref(), Some("303"));
        assert_eq!(ocr.number_confidence, 1);
    }

    #[test]
    fn test_parse_bottom_fraction_format() {
        // NNN/NNN after normalisation → adjacent pair.
        let text = "186 274 U BFZ EN KIERAN YANNER 2015 WIZARDS";
        let ocr = parse_bottom(text);
        assert_eq!(ocr.set_code.as_deref(), Some("BFZ"));
        assert_eq!(ocr.collector_number.as_deref(), Some("186"));
        assert_eq!(ocr.number_confidence, 2);
    }

    #[test]
    fn test_parse_bottom_fraction_padded() {
        let text = "058 285 U KHM EN SAM ROWAN 2021 WIZARDS";
        let ocr = parse_bottom(text);
        assert_eq!(ocr.set_code.as_deref(), Some("KHM"));
        assert_eq!(ocr.collector_number.as_deref(), Some("58"));
        assert_eq!(ocr.number_confidence, 2);
    }

    #[test]
    fn test_parse_bottom_year_not_collector() {
        let text = "C 0031 ECL EN PAOLO PARENTE 2026 WIZARDS";
        let ocr = parse_bottom(text);
        assert_eq!(ocr.collector_number.as_deref(), Some("31"));
    }

    #[test]
    fn test_parse_bottom_noisy_ocr() {
        // Real noise tokens before the actual collector number.
        let text = "N Y 3 1 1 1 373 C 0002 ECL EN NL HAMM 2026 WZ C";
        let ocr = parse_bottom(text);
        assert_eq!(ocr.set_code.as_deref(), Some("ECL"));
        assert_eq!(ocr.collector_number.as_deref(), Some("2"));
    }

    #[test]
    fn test_parse_bottom_star_separator() {
        // BLC uses ★ between set code and lang — normalize_bottom converts it to space.
        let text = normalize_bottom("M 0005 BLC\u{2605}EN STEVEN BELLEDIN 2024 WIZARDS");
        let ocr = parse_bottom(&text);
        assert_eq!(ocr.set_code.as_deref(), Some("BLC"));
        assert_eq!(ocr.collector_number.as_deref(), Some("5"));
    }

    #[test]
    fn test_parse_bottom_partial_no_set() {
        // Number present but set code absent → set_code=None, number still extracted.
        let text = "59 155 BORG NORTHERN GUIDE 1993 2006 WIZARDS COAST INC";
        let ocr = parse_bottom(text);
        assert_eq!(ocr.set_code, None);
        assert_eq!(ocr.collector_number.as_deref(), Some("59"));
        assert_eq!(ocr.number_confidence, 2);
    }

    #[test]
    fn test_parse_top_simple() {
        assert_eq!(
            parse_top_candidates("  Rooftop Percher  \n\nsome other text").first().cloned(),
            Some("Rooftop Percher".to_string())
        );
    }

    #[test]
    fn test_parse_top_noisy_ocr() {
        assert_eq!(
            parse_top_candidates("I& Rooftop Percher ) I\n").first().cloned(),
            Some("Rooftop Percher".to_string())
        );
        assert_eq!(
            parse_top_candidates("Ajani, Outland Chaperone (1 # 3%\n").first().cloned(),
            Some("Ajani, Outland Chaperone".to_string())
        );
        assert_eq!(
            parse_top_candidates("I( Reluctant Dounguard , I\n").first().cloned(),
            Some("Reluctant Dounguard".to_string())
        );
    }
}
