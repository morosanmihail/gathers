use image::DynamicImage;

/// Extracts a card from an image — normalised to 672×936.
pub trait Segmenter: Send + Sync {
    fn name(&self) -> &str;
    fn extract(&self, img: &DynamicImage) -> Option<DynamicImage>;
}

// ── Hough (existing) ──────────────────────────────────────────────────────────

/// Wraps the existing Hough-line perspective-correction segmenter.
pub struct HoughSegmenter;

impl Segmenter for HoughSegmenter {
    fn name(&self) -> &str { "hough" }
    fn extract(&self, img: &DynamicImage) -> Option<DynamicImage> {
        crate::extract_card(img)
    }
}

// ── OpenCV contour ────────────────────────────────────────────────────────────

#[cfg(feature = "opencv-seg")]
mod opencv_seg;
#[cfg(feature = "opencv-seg")]
pub use opencv_seg::OpenCvSegmenter;

// ── ONNX (YOLOv8) ────────────────────────────────────────────────────────────

#[cfg(feature = "onnx-seg")]
mod onnx_seg;
#[cfg(feature = "onnx-seg")]
pub use onnx_seg::OnnxSegmenter;
