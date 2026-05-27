use image::{DynamicImage, GenericImageView};
use opencv::{
    core::{AlgorithmHint, Mat, Point, Point2f, Scalar, Size, Vector, BORDER_CONSTANT, BORDER_DEFAULT, CV_8UC3},
    imgproc::{
        self, CHAIN_APPROX_SIMPLE, COLOR_RGB2GRAY, INTER_LINEAR, MORPH_RECT, RETR_EXTERNAL,
    },
    prelude::*,
};

const OUT_W: i32 = 672;
const OUT_H: i32 = 936;
const CARD_RATIO: f64 = 63.0 / 88.0;
const RATIO_TOLERANCE: f64 = 0.25;

pub struct OpenCvSegmenter;

impl super::Segmenter for OpenCvSegmenter {
    fn name(&self) -> &str { "opencv-contour" }

    fn extract(&self, img: &DynamicImage) -> Option<DynamicImage> {
        match extract_opencv(img) {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("OpenCV segmentation failed: {e}");
                None
            }
        }
    }
}

fn extract_opencv(img: &DynamicImage) -> opencv::Result<Option<DynamicImage>> {
    let mat = to_mat(img)?;

    let mut gray = Mat::default();
    imgproc::cvt_color(&mat, &mut gray, COLOR_RGB2GRAY, 0, AlgorithmHint::ALGO_HINT_DEFAULT)?;

    let mut blurred = Mat::default();
    imgproc::gaussian_blur(&gray, &mut blurred, Size::new(5, 5), 0.0, 0.0, BORDER_DEFAULT, AlgorithmHint::ALGO_HINT_DEFAULT)?;

    let mut edges = Mat::default();
    imgproc::canny(&blurred, &mut edges, 20.0, 60.0, 3, false)?;

    // Dilate to bridge small gaps in card border edges.
    let kernel = imgproc::get_structuring_element(
        MORPH_RECT,
        Size::new(3, 3),
        Point::new(-1, -1),
    )?;
    let mut dilated = Mat::default();
    imgproc::dilate(
        &edges, &mut dilated, &kernel,
        Point::new(-1, -1), 2,
        BORDER_CONSTANT, Scalar::default(),
    )?;

    let mut contours: Vector<Vector<Point>> = Vector::new();
    imgproc::find_contours(
        &dilated, &mut contours,
        RETR_EXTERNAL, CHAIN_APPROX_SIMPLE, Point::new(0, 0),
    )?;

    let (img_h, img_w) = (img.height() as f64, img.width() as f64);
    let min_area = img_w * img_h * 0.02;

    // Try progressively looser polygon approximations until one yields 4 corners.
    let epsilons = [0.02f64, 0.04, 0.06, 0.08];

    let best = contours.iter()
        .filter(|c| imgproc::contour_area(c, false).unwrap_or(0.0) >= min_area)
        .filter_map(|c| {
            let peri = imgproc::arc_length(&c, true).ok()?;
            let pts = epsilons.iter().find_map(|&eps| {
                let mut approx: Vector<Point> = Vector::new();
                imgproc::approx_poly_dp(&c, &mut approx, eps * peri, true).ok()?;
                if approx.len() == 4 { Some(approx.to_vec()) } else { None }
            })?;
            if !is_card_aspect(&pts) {
                return None;
            }
            let area = imgproc::contour_area(&c, false).ok()?;
            Some((pts, area))
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let (corners, _area) = match best {
        Some(b) => b,
        None => return Ok(None),
    };

    let ordered = order_corners(&corners);

    let src: Vector<Point2f> = Vector::from_iter([
        Point2f::new(ordered[0].x as f32, ordered[0].y as f32),
        Point2f::new(ordered[1].x as f32, ordered[1].y as f32),
        Point2f::new(ordered[2].x as f32, ordered[2].y as f32),
        Point2f::new(ordered[3].x as f32, ordered[3].y as f32),
    ]);
    let dst: Vector<Point2f> = Vector::from_iter([
        Point2f::new(0.0, 0.0),
        Point2f::new(OUT_W as f32 - 1.0, 0.0),
        Point2f::new(OUT_W as f32 - 1.0, OUT_H as f32 - 1.0),
        Point2f::new(0.0, OUT_H as f32 - 1.0),
    ]);

    let m = imgproc::get_perspective_transform(&src, &dst, opencv::core::DECOMP_LU)?;

    let mut warped = Mat::default();
    imgproc::warp_perspective(
        &mat, &mut warped, &m,
        Size::new(OUT_W, OUT_H),
        INTER_LINEAR, BORDER_CONSTANT, Scalar::default(),
    )?;

    Ok(Some(from_mat(&warped)?))
}

// ── Mat ↔ DynamicImage ────────────────────────────────────────────────────────

/// RGB DynamicImage → BGR Mat (OpenCV native order).
fn to_mat(img: &DynamicImage) -> opencv::Result<Mat> {
    let rgb = img.to_rgb8();
    let (w, h) = img.dimensions();
    let mut mat = Mat::new_rows_cols_with_default(
        h as i32, w as i32, CV_8UC3, Scalar::all(0.0),
    )?;
    let data = mat.data_bytes_mut()?;
    for (i, pixel) in rgb.pixels().enumerate() {
        let off = i * 3;
        data[off]     = pixel[2]; // B
        data[off + 1] = pixel[1]; // G
        data[off + 2] = pixel[0]; // R
    }
    Ok(mat)
}

/// BGR Mat → RGB DynamicImage.
fn from_mat(mat: &Mat) -> opencv::Result<DynamicImage> {
    let rows = mat.rows() as u32;
    let cols = mat.cols() as u32;
    let data = mat.data_bytes()?;
    let rgb: Vec<u8> = data.chunks(3)
        .flat_map(|c| [c[2], c[1], c[0]])
        .collect();
    let img = image::RgbImage::from_raw(cols, rows, rgb)
        .ok_or_else(|| opencv::Error::new(-1, "mat→image conversion failed"))?;
    Ok(DynamicImage::ImageRgb8(img))
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

fn is_card_aspect(pts: &[Point]) -> bool {
    let mut lengths = [0.0f64; 4];
    for i in 0..4 {
        let j = (i + 1) % 4;
        let dx = (pts[j].x - pts[i].x) as f64;
        let dy = (pts[j].y - pts[i].y) as f64;
        lengths[i] = (dx * dx + dy * dy).sqrt();
    }
    lengths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let short = (lengths[0] + lengths[1]) / 2.0;
    let long_ = (lengths[2] + lengths[3]) / 2.0;
    if long_ < 1.0 { return false; }
    let ratio = short / long_;
    (ratio - CARD_RATIO).abs() < RATIO_TOLERANCE
}

/// Order corners as [TL, TR, BR, BL] using sum/difference of coordinates.
fn order_corners(pts: &[Point]) -> [Point; 4] {
    let tl = *pts.iter().min_by_key(|p| p.x + p.y).unwrap();
    let tr = *pts.iter().max_by_key(|p| p.x - p.y).unwrap();
    let br = *pts.iter().max_by_key(|p| p.x + p.y).unwrap();
    let bl = *pts.iter().min_by_key(|p| p.x - p.y).unwrap();
    [tl, tr, br, bl]
}
