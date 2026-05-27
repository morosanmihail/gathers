use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ort::{
    session::Session,
    value::TensorRef,
};
use std::{path::{Path, PathBuf}, sync::Mutex};

const INPUT_W: u32 = 640;
const INPUT_H: u32 = 640;
const CARD_RATIO: f32 = 63.0 / 88.0;
const RATIO_TOLERANCE: f32 = 0.25;
const CONFIDENCE_THRESHOLD: f32 = 0.10;

/// Segmenter backed by a YOLOv8 ONNX detection model.
///
/// Export from Ultralytics with:
///   yolo export model=yolov8n.pt format=onnx
///
/// Set `YOLO_MODEL_PATH` env var or construct with an explicit path.
pub struct OnnxSegmenter {
    session: Mutex<Session>,
    output_name: String,
}

impl OnnxSegmenter {
    pub fn from_env() -> eyre::Result<Self> {
        let path = std::env::var("YOLO_MODEL_PATH")
            .map(PathBuf::from)
            .map_err(|_| eyre::eyre!("YOLO_MODEL_PATH not set"))?;
        Self::new(&path)
    }

    pub fn new(model_path: &Path) -> eyre::Result<Self> {
        let session = Session::builder()?.commit_from_file(model_path)?;
        let output_name = session
            .outputs()
            .first()
            .map(|o| o.name().to_string())
            .unwrap_or_else(|| "output0".to_string());
        tracing::debug!("OnnxSegmenter loaded model={model_path:?} output={output_name:?}");
        Ok(Self { session: Mutex::new(session), output_name })
    }
}

impl super::Segmenter for OnnxSegmenter {
    fn name(&self) -> &str { "onnx-yolo" }

    fn extract(&self, img: &DynamicImage) -> Option<DynamicImage> {
        match detect(img, &mut self.session.lock().unwrap(), &self.output_name) {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("ONNX segmentation failed: {e}");
                None
            }
        }
    }
}

struct Detection {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    conf: f32,
}

fn detect(
    img: &DynamicImage,
    session: &mut Session,
    output_name: &str,
) -> eyre::Result<Option<DynamicImage>> {
    let (orig_w, orig_h) = img.dimensions();

    // Preprocess: resize to 640×640, RGB NCHW float32 [0,1].
    let resized = img.resize_exact(INPUT_W, INPUT_H, FilterType::Lanczos3).to_rgb8();
    let n_pixels = (INPUT_W * INPUT_H) as usize;
    let mut data = vec![0f32; 3 * n_pixels];
    for (i, pixel) in resized.pixels().enumerate() {
        data[i]                 = pixel[0] as f32 / 255.0; // R channel
        data[n_pixels + i]     = pixel[1] as f32 / 255.0; // G channel
        data[2 * n_pixels + i] = pixel[2] as f32 / 255.0; // B channel
    }

    let shape = [1usize, 3, INPUT_H as usize, INPUT_W as usize];
    let input_tensor = TensorRef::from_array_view((shape.as_slice(), data.as_slice()))?;
    let outputs = session.run(ort::inputs!["images" => input_tensor])?;

    let (out_shape, out_data) = outputs[output_name].try_extract_tensor::<f32>()?;

    // YOLOv8 output: [1, num_features, num_anchors]
    // num_features = 4 (cx,cy,w,h) + num_classes
    if out_shape.len() != 3 || out_shape[0] != 1 {
        eyre::bail!("unexpected ONNX output shape: {out_shape:?}");
    }
    let num_features = out_shape[1] as usize;
    let num_anchors = out_shape[2] as usize;

    let scale_x = orig_w as f32 / INPUT_W as f32;
    let scale_y = orig_h as f32 / INPUT_H as f32;

    // out_data is row-major [1, num_features, num_anchors].
    // Element [0, f, i] = out_data[f * num_anchors + i].
    let elem = |f: usize, i: usize| out_data[f * num_anchors + i];

    let mut detections: Vec<Detection> = Vec::new();
    for i in 0..num_anchors {
        let cx = elem(0, i) * scale_x;
        let cy = elem(1, i) * scale_y;
        let bw = elem(2, i) * scale_x;
        let bh = elem(3, i) * scale_y;

        // Max class score across remaining features.
        let conf = (4..num_features)
            .map(|f| elem(f, i))
            .fold(f32::NEG_INFINITY, f32::max);

        if conf < CONFIDENCE_THRESHOLD {
            continue;
        }

        // Filter by card aspect ratio (portrait or landscape).
        let short = bw.min(bh);
        let long_ = bw.max(bh).max(1.0);
        let ratio = short / long_;
        if (ratio - CARD_RATIO).abs() > RATIO_TOLERANCE {
            continue;
        }

        detections.push(Detection {
            x1: (cx - bw / 2.0).max(0.0),
            y1: (cy - bh / 2.0).max(0.0),
            x2: (cx + bw / 2.0).min(orig_w as f32),
            y2: (cy + bh / 2.0).min(orig_h as f32),
            conf,
        });
    }

    // Highest-confidence card-shaped detection.
    detections.sort_by(|a, b| b.conf.partial_cmp(&a.conf).unwrap_or(std::cmp::Ordering::Equal));
    let det = match detections.into_iter().next() {
        Some(d) => d,
        None => return Ok(None),
    };

    let x = det.x1 as u32;
    let y = det.y1 as u32;
    let w = ((det.x2 - det.x1) as u32).min(orig_w.saturating_sub(x));
    let h = ((det.y2 - det.y1) as u32).min(orig_h.saturating_sub(y));

    if w == 0 || h == 0 {
        return Ok(None);
    }

    let cropped = img.crop_imm(x, y, w, h);
    let normalised = cropped.resize_exact(672, 936, FilterType::Lanczos3);
    tracing::debug!("ONNX card detected conf={:.2} at [{x},{y},{w}×{h}]", det.conf);
    Ok(Some(normalised))
}
