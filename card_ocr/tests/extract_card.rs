use card_ocr::{HoughSegmenter, Segmenter, extract_card};
use std::path::PathBuf;

fn cards_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/cards")
}

fn assert_extracted(filename: &str) {
    let path = cards_dir().join(format!("shape/{filename}"));
    if !path.exists() {
        eprintln!("SKIP: {filename} not found");
        return;
    }
    let img = image::open(&path).unwrap_or_else(|e| panic!("Failed to open {filename}: {e}"));
    let result = extract_card(&img);
    assert!(result.is_some(), "extract_card returned None for {filename}");
    let extracted = result.unwrap();
    assert_eq!(extracted.width(), 672, "{filename}: expected width 672");
    assert_eq!(extracted.height(), 936, "{filename}: expected height 936");
}

#[test]
fn test_extract_card_shape1() {
    assert_extracted("shape1.png");
}

#[test]
fn test_extract_card_shape2() {
    assert_extracted("shape2.jpeg");
}

// ── Segmenter comparison ──────────────────────────────────────────────────────

fn run_segmenter(segmenter: &dyn Segmenter, filename: &str) -> Option<image::DynamicImage> {
    let path = cards_dir().join(format!("shape/{filename}"));
    if !path.exists() {
        eprintln!("SKIP: {filename} not found");
        return None;
    }
    let img = image::open(&path).unwrap_or_else(|e| panic!("Failed to open {filename}: {e}"));
    segmenter.extract(&img)
}

fn assert_segmenter(segmenter: &dyn Segmenter, filename: &str) {
    let result = run_segmenter(segmenter, filename);
    if result.is_none() {
        eprintln!("  {} → None for {filename}", segmenter.name());
        return;
    }
    let extracted = result.unwrap();
    assert_eq!(extracted.width(), 672, "{} {filename}: width", segmenter.name());
    assert_eq!(extracted.height(), 936, "{} {filename}: height", segmenter.name());
    println!("  {} → ok (672×936) for {filename}", segmenter.name());
}

#[test]
fn test_hough_segmenter_shape1() {
    assert_segmenter(&HoughSegmenter, "shape1.png");
}

#[test]
fn test_hough_segmenter_shape2() {
    assert_segmenter(&HoughSegmenter, "shape2.jpeg");
}

#[cfg(feature = "opencv-seg")]
mod opencv_tests {
    use super::*;
    use card_ocr::OpenCvSegmenter;

    #[test]
    fn test_opencv_segmenter_shape1() {
        assert_segmenter(&OpenCvSegmenter, "shape1.png");
    }

    #[test]
    fn test_opencv_segmenter_shape2() {
        assert_segmenter(&OpenCvSegmenter, "shape2.jpeg");
    }
}

#[cfg(feature = "onnx-seg")]
mod onnx_tests {
    use super::*;
    use card_ocr::OnnxSegmenter;
    use std::path::PathBuf;

    fn model_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/yolov8n.onnx")
    }

    #[test]
    fn test_onnx_segmenter_shape1() {
        let path = model_path();
        if !path.exists() {
            eprintln!("SKIP: yolov8n.onnx not found at {path:?}");
            return;
        }
        let seg = OnnxSegmenter::new(&path).expect("load model");
        assert_segmenter(&seg, "shape1.png");
    }

    #[test]
    fn test_onnx_segmenter_shape2() {
        let path = model_path();
        if !path.exists() {
            eprintln!("SKIP: yolov8n.onnx not found at {path:?}");
            return;
        }
        let seg = OnnxSegmenter::new(&path).expect("load model");
        assert_segmenter(&seg, "shape2.jpeg");
    }
}

#[test]
fn debug_shape2() {
    use imageproc::{edges::canny, filter::gaussian_blur_f32, hough::{LineDetectionOptions, detect_lines}};
    let path = cards_dir().join("shape/shape2.jpeg");
    if !path.exists() { eprintln!("SKIP"); return; }
    let img = image::open(&path).expect("open");
    let (w, h) = (img.width(), img.height());
    println!("{}×{}", w, h);
    let gray = img.to_luma8();

    for (sigma, lo, hi) in [(2.0f32, 30.0f32, 100.0f32), (3.0, 15.0, 50.0)] {
        println!("\n--- sigma={sigma} canny=({lo},{hi}) ---");
        let blurred = gaussian_blur_f32(&gray, sigma);
        let edges = canny(&blurred, lo, hi);
        edges.save(format!("/tmp/shape2_edges_s{sigma:.0}.png")).ok();
        for &thr in &[50u32, 80, 100, 150] {
            let lines = detect_lines(&edges, LineDetectionOptions { vote_threshold: thr, suppression_radius: 8 });
            println!("thr={thr}: {} lines", lines.len());
            if lines.len() <= 25 {
                let mut v = lines.clone();
                v.sort_by_key(|l| l.angle_in_degrees);
                for l in &v { println!("  angle={:3}° r={:.1}", l.angle_in_degrees, l.r); }
            }
        }
    }
}
