use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbImage};
use imageproc::{
    edges::canny,
    filter::gaussian_blur_f32,
    geometric_transformations::{Interpolation, Projection, warp_into},
    hough::{LineDetectionOptions, PolarLine, detect_lines},
    point::Point,
};

const OUT_W: u32 = 672;
const OUT_H: u32 = 936;
/// MTG card dimensions: 63 mm wide × 88 mm tall.
const CARD_RATIO: f32 = 63.0 / 88.0;
const RATIO_TOLERANCE: f32 = 0.15;

/// Diagnostic info from an `extract_card` attempt.
pub struct ExtractionStatus {
    /// Short human-readable description of why extraction succeeded or failed.
    pub message: String,
    /// Number of Hough lines detected.
    pub line_count: usize,
    /// Vote threshold used for Hough detection.
    pub vote_threshold: u32,
}

/// Like `extract_card` but also returns a status message useful for live debugging.
pub fn extract_card_debug(img: &DynamicImage) -> (Option<DynamicImage>, ExtractionStatus) {
    let (result, status) = extract_card_inner(img);
    (result, status)
}

/// Find a Magic: The Gathering card in `img` and return it perspective-corrected
/// to a flat 672×936 image. Returns `None` if no card-shaped quad is found.
pub fn extract_card(img: &DynamicImage) -> Option<DynamicImage> {
    extract_card_inner(img).0
}

fn extract_card_inner(img: &DynamicImage) -> (Option<DynamicImage>, ExtractionStatus) {
    let (w, h) = img.dimensions();
    let rgb = img.to_rgb8();
    let gray = img.to_luma8();

    // Scale sigma with resolution so a 4K camera isn't under-smoothed.
    // Baseline: sigma=2 at ~1080p short side; double at 4K.
    let sigma = (w.min(h) as f32 / 540.0).clamp(2.0, 5.0);
    let blurred = gaussian_blur_f32(&gray, sigma);
    let edges = canny(&blurred, 30.0, 100.0);

    // 8% of the shorter dimension. 15% was too high for 4K cameras where a
    // held card can occupy a small fraction of the frame.
    let vote_threshold = (w.min(h) * 8 / 100).max(40) as u32;
    let lines = detect_lines(
        &edges,
        LineDetectionOptions { vote_threshold, suppression_radius: 8 },
    );

    let line_count = lines.len();
    let mk = |msg: &str| ExtractionStatus { message: msg.to_string(), line_count, vote_threshold };

    // Count edge pixels so we can distinguish "Canny found nothing" from
    // "Canny found edges but Hough threshold too high".
    let edge_px: u32 = edges.pixels().map(|p| if p.0[0] > 0 { 1 } else { 0 }).sum();

    tracing::debug!("Hough: {} lines (threshold={}, edge_px={})", line_count, vote_threshold, edge_px);
    if line_count < 4 {
        return (None, mk(&format!(
            "too few lines ({line_count}, need ≥4; hough_thr={vote_threshold}, sigma={sigma:.1}, edge_px={edge_px}, img={h}×{w})"
        )));
    }

    let primary = find_card_corners(&lines);
    let primary_ok = primary.filter(|c| is_card_aspect(c, CARD_RATIO));

    let (corners, how) = if let Some(c) = primary_ok {
        (c, "primary")
    } else if let Some(c) = find_card_corners_infer(&lines, w, h) {
        (c, "inferred")
    } else {
        let reason = if find_card_corners(&lines).is_none() {
            "no perpendicular line pair"
        } else {
            "wrong aspect ratio"
        };
        return (None, mk(&format!("{reason} ({line_count} lines)")));
    };

    tracing::debug!("Card corners ({how}): {:?}", corners);

    let ordered = order_corners(corners);
    let src: [(f32, f32); 4] = [
        (ordered[0].x as f32, ordered[0].y as f32),
        (ordered[1].x as f32, ordered[1].y as f32),
        (ordered[2].x as f32, ordered[2].y as f32),
        (ordered[3].x as f32, ordered[3].y as f32),
    ];
    let dst: [(f32, f32); 4] = [
        (0.0, 0.0),
        (OUT_W as f32 - 1.0, 0.0),
        (OUT_W as f32 - 1.0, OUT_H as f32 - 1.0),
        (0.0, OUT_H as f32 - 1.0),
    ];

    let Some(proj) = Projection::from_control_points(src, dst) else {
        return (None, mk("projection failed"));
    };
    let mut out: RgbImage = ImageBuffer::new(OUT_W, OUT_H);
    warp_into(&rgb, &proj, Interpolation::Bilinear, Rgb([0u8, 0, 0]), &mut out);
    let status = mk(&format!("ok ({how}, {line_count} lines)"));
    (Some(DynamicImage::ImageRgb8(out)), status)
}

/// Group detected Hough lines into two perpendicular clusters and return
/// the four intersection points of the outermost pair from each cluster.
fn find_card_corners(lines: &[PolarLine]) -> Option<[Point<i32>; 4]> {
    let (group_a, group_b, angle_a, angle_b) = best_angle_groups(lines)?;

    // Need at least 2 lines per group to form a rectangle without inference.
    if group_a.len() < 2 || group_b.len() < 2 {
        return None;
    }

    // Outermost pair: min and max normalized-r in each group.
    let a1 = *group_a.iter().min_by(|x, y| norm_r(x, angle_a).partial_cmp(&norm_r(y, angle_a)).unwrap())?;
    let a2 = *group_a.iter().max_by(|x, y| norm_r(x, angle_a).partial_cmp(&norm_r(y, angle_a)).unwrap())?;
    let b1 = *group_b.iter().min_by(|x, y| norm_r(x, angle_b).partial_cmp(&norm_r(y, angle_b)).unwrap())?;
    let b2 = *group_b.iter().max_by(|x, y| norm_r(x, angle_b).partial_cmp(&norm_r(y, angle_b)).unwrap())?;

    let c1 = line_intersect(a1, b1)?;
    let c2 = line_intersect(a1, b2)?;
    let c3 = line_intersect(a2, b2)?;
    let c4 = line_intersect(a2, b1)?;
    Some([c1, c2, c3, c4])
}

/// Fallback: when one pair of parallel edges is partially occluded (e.g. hand
/// blocking the bottom of the card), infer the missing far edge from the
/// detected near edge + the known card aspect ratio.
fn find_card_corners_infer(lines: &[PolarLine], img_w: u32, img_h: u32) -> Option<[Point<i32>; 4]> {
    let (group_a, group_b, angle_a, angle_b) = best_angle_groups(lines)?;

    let a1 = *group_a.iter().min_by(|x, y| norm_r(x, angle_a).partial_cmp(&norm_r(y, angle_a)).unwrap())?;
    let a2 = *group_a.iter().max_by(|x, y| norm_r(x, angle_a).partial_cmp(&norm_r(y, angle_a)).unwrap())?;
    let b1 = *group_b.iter().min_by(|x, y| norm_r(x, angle_b).partial_cmp(&norm_r(y, angle_b)).unwrap())?;
    let b2 = *group_b.iter().max_by(|x, y| norm_r(x, angle_b).partial_cmp(&norm_r(y, angle_b)).unwrap())?;

    let span_a = (norm_r(&a2, angle_a) - norm_r(&a1, angle_a)).abs();
    let span_b = (norm_r(&b2, angle_b) - norm_r(&b1, angle_b)).abs();
    let max_dim = img_w.max(img_h) as f32;

    // Try inferring a missing far edge in group_b, then group_a.
    // For each, try both card orientations (portrait/landscape).
    for &scale in &[88.0_f32 / 63.0, 63.0_f32 / 88.0] {
        // group_b far edge inferred from span_a
        if span_b < span_a * scale * 0.5 {
            let near_r = norm_r(&b1, angle_b);
            let far_r = near_r + span_a * scale;
            if far_r > 0.0 && far_r < max_dim * 1.1 {
                let synth_b2 = PolarLine { r: far_r, angle_in_degrees: b1.angle_in_degrees };
                if let Some(corners) = quad(a1, a2, b1, synth_b2) {
                    if is_card_aspect(&corners, CARD_RATIO) {
                        tracing::debug!("Inferred far-B r={far_r:.1}");
                        return Some(corners);
                    }
                }
            }
        }
        // group_a far edge inferred from span_b
        if span_a < span_b * scale * 0.5 {
            let near_r = norm_r(&a1, angle_a);
            let far_r = near_r + span_b * scale;
            if far_r > 0.0 && far_r < max_dim * 1.1 {
                let synth_a2 = PolarLine { r: far_r, angle_in_degrees: a1.angle_in_degrees };
                if let Some(corners) = quad(a1, synth_a2, b1, b2) {
                    if is_card_aspect(&corners, CARD_RATIO) {
                        tracing::debug!("Inferred far-A r={far_r:.1}");
                        return Some(corners);
                    }
                }
            }
        }
    }
    None
}

fn quad(a1: PolarLine, a2: PolarLine, b1: PolarLine, b2: PolarLine) -> Option<[Point<i32>; 4]> {
    Some([
        line_intersect(a1, b1)?,
        line_intersect(a1, b2)?,
        line_intersect(a2, b2)?,
        line_intersect(a2, b1)?,
    ])
}

/// Normalise a line's r to the canonical angle, correcting sign for lines
/// that wrap around the 0°/180° boundary (e.g. angle=179° near canonical=0°).
fn norm_r(l: &PolarLine, canonical: u32) -> f32 {
    let diff = (l.angle_in_degrees as f32 - canonical as f32).to_radians();
    l.r * diff.cos()
}

/// Find the best perpendicular angle pair and return the two line groups.
fn best_angle_groups(lines: &[PolarLine]) -> Option<(Vec<PolarLine>, Vec<PolarLine>, u32, u32)> {
    let mut hist = [0u32; 180];
    for l in lines {
        hist[l.angle_in_degrees as usize] += 1;
    }

    let window = 8u32;
    let mut best_score = 0u32;
    let mut best_a = 0u32;
    for a in 0u32..90 {
        let ca = angle_sum(&hist, a, window);
        let cb = angle_sum(&hist, a + 90, window);
        let score = ca * cb;
        if score > best_score {
            best_score = score;
            best_a = a;
        }
    }
    if best_score == 0 {
        return None;
    }

    let angle_a = best_a;
    let angle_b = best_a + 90;

    let group_a: Vec<PolarLine> = lines
        .iter()
        .filter(|l| angle_near(l.angle_in_degrees, angle_a, window))
        .copied()
        .collect();
    let group_b: Vec<PolarLine> = lines
        .iter()
        .filter(|l| angle_near(l.angle_in_degrees, angle_b, window))
        .copied()
        .collect();

    tracing::debug!("Angle A={angle_a} ({}), B={angle_b} ({})", group_a.len(), group_b.len());

    if group_a.is_empty() || group_b.is_empty() {
        return None;
    }

    Some((group_a, group_b, angle_a, angle_b))
}

/// Solve the 2×2 system x·cos θ + y·sin θ = r for two polar lines.
fn line_intersect(a: PolarLine, b: PolarLine) -> Option<Point<i32>> {
    let t1 = (a.angle_in_degrees as f32).to_radians();
    let t2 = (b.angle_in_degrees as f32).to_radians();
    let (c1, s1) = (t1.cos(), t1.sin());
    let (c2, s2) = (t2.cos(), t2.sin());
    let det = c1 * s2 - c2 * s1;
    if det.abs() < 1e-6 {
        return None; // parallel
    }
    let x = (a.r * s2 - b.r * s1) / det;
    let y = (b.r * c1 - a.r * c2) / det;
    Some(Point::new(x.round() as i32, y.round() as i32))
}

fn angle_sum(hist: &[u32; 180], center: u32, radius: u32) -> u32 {
    (0..=radius * 2)
        .map(|i| hist[((center + i + 180 - radius) % 180) as usize])
        .sum()
}

fn angle_near(a: u32, center: u32, radius: u32) -> bool {
    let diff = (a as i32 - center as i32).rem_euclid(180).min((center as i32 - a as i32).rem_euclid(180));
    diff <= radius as i32
}

fn is_card_aspect(pts: &[Point<i32>; 4], target_ratio: f32) -> bool {
    let mut lengths = [0.0f32; 4];
    for i in 0..4 {
        let j = (i + 1) % 4;
        let dx = (pts[j].x - pts[i].x) as f32;
        let dy = (pts[j].y - pts[i].y) as f32;
        lengths[i] = (dx * dx + dy * dy).sqrt();
    }
    lengths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let short_avg = (lengths[0] + lengths[1]) / 2.0;
    let long_avg = (lengths[2] + lengths[3]) / 2.0;
    if long_avg < 1.0 {
        return false;
    }
    let ratio = short_avg / long_avg;
    (ratio - target_ratio).abs() < RATIO_TOLERANCE
}

/// Order four corners as [TL, TR, BR, BL] using coordinate sums/differences.
fn order_corners(pts: [Point<i32>; 4]) -> [Point<i32>; 4] {
    let tl = pts.iter().copied().min_by_key(|p| p.x + p.y).unwrap();
    let tr = pts.iter().copied().max_by_key(|p| p.x - p.y).unwrap();
    let br = pts.iter().copied().max_by_key(|p| p.x + p.y).unwrap();
    let bl = pts.iter().copied().min_by_key(|p| p.x - p.y).unwrap();
    [tl, tr, br, bl]
}
