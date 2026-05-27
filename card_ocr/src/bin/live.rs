// card_ocr_live — webcam card scanner with Bevy UI.
//
// Usage: card_ocr_live [db_path]
//   db_path   Optional path to AllPrintings.db.  Defaults to
//             ~/.local/share/gathers/DB/AllPrintings.db.
//             If missing, card identification is skipped; extraction still runs.
//
// Layout:  70 % left = live webcam | 30 % right = identified-card log + debug panel
//
// Architecture:
//   Capture thread   — grabs frames from camera, writes to Arc<Mutex<LatestRgb>>
//   OCR thread       — clones latest frame, runs extract_card + identify_card_image
//   Bevy main thread — reads shared frame into GPU texture, receives OCR results

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
};
use card_ocr::identify_card_image;
#[cfg(any(feature = "opencv-seg", feature = "onnx-seg"))]
use card_ocr::Segmenter;
#[cfg(feature = "opencv-seg")]
use card_ocr::OpenCvSegmenter;
#[cfg(feature = "onnx-seg")]
use card_ocr::OnnxSegmenter;
use image::{DynamicImage, ImageBuffer};
use models::{Card, CardTrait};
use nokhwa::{
    Camera,
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};
use retrieval::MagicSQLiteRetrievalSystem;
use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

const LOG_MAX: usize = 20;

// ── Shared debug state (written by OCR thread, read by Bevy) ─────────────────

#[derive(Default, Clone)]
struct DebugState {
    /// Which segmenter produced the image sent to OCR ("opencv", "onnx", or "—").
    segmenter_used: String,
    /// OCR cycles completed (ever-increasing).
    cycles: u64,
    /// Cycles per second, updated roughly every second.
    cps: f32,
    /// Cards successfully identified so far.
    cards_found: u64,
    /// Last identification result string (or "—" if none).
    last_result: String,
}

// ── Resources ─────────────────────────────────────────────────────────────────

/// Latest raw RGB frame from the capture thread.
#[derive(Resource, Clone)]
struct LatestFrame(Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>);

/// Receives formatted card strings from the OCR thread.
#[derive(Resource)]
struct OcrRx(Mutex<mpsc::Receiver<String>>);

/// Shared debug info from OCR thread.
#[derive(Resource, Clone)]
struct SharedDebug(Arc<Mutex<DebugState>>);

/// Handle to the live webcam GPU texture.
#[derive(Resource)]
struct WebcamTexture(Handle<Image>);

/// Cumulative card log (oldest at top, newest at bottom).
#[derive(Resource, Default)]
struct CardLog(Vec<String>);

/// Camera dimensions determined at startup.
#[derive(Resource)]
struct CamSize(u32, u32);

// ── Component markers ─────────────────────────────────────────────────────────

#[derive(Component)]
struct LogText;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct WebcamImageNode;

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let db_path: Option<PathBuf> = env::args().nth(1).map(PathBuf::from).or_else(|| {
        let home = env::var("HOME").unwrap_or_else(|_| "/root".into());
        let p = PathBuf::from(home).join(".local/share/gathers/DB/AllPrintings.db");
        if p.exists() { Some(p) } else { None }
    });

    if db_path.is_none() {
        eprintln!("Note: AllPrintings.db not found — card identification disabled.");
        eprintln!("      Pass db path as first argument or set MTG_DB_PATH.");
    }

    let shared_frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>> = Arc::new(Mutex::new(None));
    let shared_debug: Arc<Mutex<DebugState>> = Arc::new(Mutex::new(DebugState::default()));

    {
        let frame = shared_frame.clone();
        thread::spawn(move || capture_thread(frame));
    }

    let (ocr_tx, ocr_rx) = mpsc::channel::<String>();
    {
        let frame = shared_frame.clone();
        let debug = shared_debug.clone();
        thread::spawn(move || ocr_thread(frame, ocr_tx, db_path, debug));
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Card OCR Live".to_string(),
                resolution: (1280., 720.).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(LatestFrame(shared_frame))
        .insert_resource(OcrRx(Mutex::new(ocr_rx)))
        .insert_resource(SharedDebug(shared_debug))
        .insert_resource(CardLog::default())
        // Texture starts at 640×480; update_webcam_texture resizes on first frame.
        .insert_resource(CamSize(640, 480))
        .add_systems(Startup, setup)
        .add_systems(Update, (update_webcam_texture, poll_ocr, sync_log_text, sync_debug_text))
        .run();
}

// ── Capture thread ────────────────────────────────────────────────────────────

fn capture_thread(frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>) {
    loop {
        let mut camera = match Camera::new(
            CameraIndex::Index(0),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate),
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Camera unavailable ({e}), retrying in 3s…");
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        };

        if let Err(e) = camera.open_stream() {
            eprintln!("Camera stream failed ({e}), retrying in 3s…");
            thread::sleep(Duration::from_secs(3));
            continue;
        }

        eprintln!("Camera opened.");

        loop {
            match camera.frame() {
                Ok(buf) => match buf.decode_image::<RgbFormat>() {
                    Ok(decoded) => {
                        let (w, h) = (decoded.width(), decoded.height());
                        let bytes = decoded.into_raw();
                        if let Ok(mut g) = frame.lock() {
                            *g = Some((bytes, w, h));
                        }
                    }
                    Err(e) => eprintln!("Frame decode error: {e}"),
                },
                Err(e) => {
                    eprintln!("Camera lost ({e}), reconnecting in 3s…");
                    thread::sleep(Duration::from_secs(3));
                    break; // back to outer loop → reopen camera
                }
            }
        }
    }
}

// ── OCR thread ────────────────────────────────────────────────────────────────

fn ocr_thread(
    frame: Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>,
    tx: mpsc::Sender<String>,
    db_path: Option<PathBuf>,
    debug: Arc<Mutex<DebugState>>,
) {
    let retrieval = db_path.as_deref().and_then(|p| {
        MagicSQLiteRetrievalSystem::new(Some(p.to_string_lossy().into_owned()), None).ok()
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let mut last_entry: Option<String> = None;
    let mut cycles: u64 = 0;
    let mut cards_found: u64 = 0;
    let mut cps_window_start = Instant::now();
    let mut cps_cycles_at_window: u64 = 0;

    loop {
        let snapshot = frame.lock().ok().and_then(|g| g.clone());

        let Some((bytes, w, h)) = snapshot else {
            thread::sleep(Duration::from_millis(50));
            continue;
        };

        let Some(buf) = ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_raw(w, h, bytes) else {
            continue;
        };
        #[allow(unused_variables)]
        let dyn_img = DynamicImage::ImageRgb8(buf);

        // Try OpenCV → ONNX; fall back to raw image if none succeed.
        #[allow(unused_mut, unused_assignments)]
        let mut segmenter_used = "—";
        #[allow(unused_mut)]
        let mut extracted: Option<DynamicImage> = None;

        #[cfg(feature = "opencv-seg")]
        if extracted.is_none() {
            if let Some(img) = OpenCvSegmenter.extract(&dyn_img) {
                segmenter_used = "opencv";
                extracted = Some(img);
            }
        }
        #[cfg(feature = "onnx-seg")]
        if extracted.is_none() {
            if let Ok(seg) = OnnxSegmenter::from_env() {
                if let Some(img) = seg.extract(&dyn_img) {
                    segmenter_used = "onnx";
                    extracted = Some(img);
                }
            }
        }

        cycles += 1;

        let elapsed = cps_window_start.elapsed().as_secs_f32();
        let cps = if elapsed >= 1.0 {
            let c = (cycles - cps_cycles_at_window) as f32 / elapsed;
            cps_window_start = Instant::now();
            cps_cycles_at_window = cycles;
            c
        } else {
            debug.lock().map(|d| d.cps).unwrap_or(0.0)
        };

        let last_result = if let Some(ref extracted) = extracted {
            if let Some(ref r) = retrieval {
                match rt.block_on(identify_card_image(extracted, r)) {
                    Ok(Some(card)) => {
                        let entry = format!(
                            "{} ({} #{})",
                            card_name(&card),
                            card.get_set(),
                            card.get_collector_number()
                        );
                        if last_entry.as_deref() != Some(&entry) {
                            last_entry = Some(entry.clone());
                            let _ = tx.send(entry.clone());
                            cards_found += 1;
                        }
                        entry
                    }
                    Ok(None) => {
                        last_entry = None;
                        "shape found, no DB match".to_string()
                    }
                    Err(e) => {
                        eprintln!("OCR error: {e}");
                        format!("OCR error: {e}")
                    }
                }
            } else {
                "no DB (extraction only)".to_string()
            }
        } else {
            "—".to_string()
        };

        if let Ok(mut d) = debug.lock() {
            d.segmenter_used = segmenter_used.to_string();
            d.cycles = cycles;
            d.cps = cps;
            d.cards_found = cards_found;
            d.last_result = last_result;
        }

    }
}

fn card_name(card: &Card) -> &str {
    match card {
        Card::Magic(c) => &c.name,
        Card::Riftbound(c) => &c.name,
        Card::Pokemon(c) => &c.name,
    }
}

// ── Bevy setup ────────────────────────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    cam: Res<CamSize>,
) {
    commands.spawn(Camera2d);

    let mut tex = Image::new_fill(
        Extent3d { width: cam.0, height: cam.1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0u8, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    tex.texture_descriptor.usage |= TextureUsages::COPY_DST;
    let handle = images.add(tex);
    commands.insert_resource(WebcamTexture(handle.clone()));

    // ── Root row ──────────────────────────────────────────────────────────────
    commands
        .spawn(Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|root| {
            // ── Left 70 %: webcam video ───────────────────────────────────────
            root.spawn(Node {
                width: Val::Percent(70.),
                height: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|video| {
                video.spawn((
                    ImageNode::new(handle),
                    Node {
                        width: Val::Percent(100.),
                        height: Val::Auto,
                        aspect_ratio: Some(4. / 3.),
                        ..default()
                    },
                    WebcamImageNode,
                ));
            });

            // ── Right 30 %: debug panel + card log ────────────────────────────
            root.spawn((
                Node {
                    width: Val::Percent(30.),
                    height: Val::Percent(100.),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.06, 0.06, 0.08)),
            ))
            .with_children(|panel| {
                // Debug section header
                panel.spawn((
                    Text::new("Debug"),
                    TextFont { font_size: 15., ..default() },
                    TextColor(Color::srgb(0.6, 0.8, 1.0)),
                    Node { margin: UiRect::bottom(Val::Px(4.)), ..default() },
                ));

                // Debug info text (updated every frame)
                panel.spawn((
                    Text::new(""),
                    TextFont { font_size: 11., ..default() },
                    TextColor(Color::srgb(0.7, 0.85, 0.7)),
                    Node { margin: UiRect::bottom(Val::Px(12.)), ..default() },
                    DebugText,
                ));

                // Divider label
                panel.spawn((
                    Text::new("Identified Cards"),
                    TextFont { font_size: 15., ..default() },
                    TextColor(Color::srgb(0.9, 0.85, 0.4)),
                    Node { margin: UiRect::bottom(Val::Px(6.)), ..default() },
                ));

                panel.spawn((
                    Text::new(""),
                    TextFont { font_size: 13., ..default() },
                    TextColor(Color::srgb(0.75, 0.95, 0.75)),
                    LogText,
                ));
            });
        });
}

// ── Update systems ────────────────────────────────────────────────────────────

fn update_webcam_texture(
    latest: Res<LatestFrame>,
    mut images: ResMut<Assets<Image>>,
    tex: Res<WebcamTexture>,
    mut cam_node: Query<&mut Node, With<WebcamImageNode>>,
) {
    let Ok(guard) = latest.0.try_lock() else { return };
    let Some((rgb, w, h)) = guard.as_ref() else { return };
    let Some(image) = images.get_mut(&tex.0) else { return };

    let expected = (*w as usize) * (*h as usize) * 4;
    if image.data.len() != expected {
        image.texture_descriptor.size =
            Extent3d { width: *w, height: *h, depth_or_array_layers: 1 };
        image.data.resize(expected, 255);

        if let Ok(mut node) = cam_node.get_single_mut() {
            node.aspect_ratio = Some(*w as f32 / *h as f32);
        }
    }

    for (dst, src) in image.data.chunks_exact_mut(4).zip(rgb.chunks_exact(3)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
        dst[3] = 255;
    }
}

fn poll_ocr(rx: Res<OcrRx>, mut log: ResMut<CardLog>) {
    let Ok(rx) = rx.0.try_lock() else { return };
    while let Ok(entry) = rx.try_recv() {
        log.0.push(entry);
        if log.0.len() > LOG_MAX {
            log.0.remove(0);
        }
    }
}

fn sync_log_text(log: Res<CardLog>, mut q: Query<&mut Text, With<LogText>>) {
    if !log.is_changed() {
        return;
    }
    if let Ok(mut t) = q.get_single_mut() {
        *t = Text::new(log.0.join("\n"));
    }
}

fn sync_debug_text(
    shared: Res<SharedDebug>,
    mut q: Query<&mut Text, With<DebugText>>,
) {
    let Ok(d) = shared.0.try_lock() else { return };
    let Ok(mut t) = q.get_single_mut() else { return };
    *t = Text::new(format!(
        "cycles:  {}\nOCR/s:   {:.1}\ncards:   {}\nsegment: {}\nlast:    {}",
        d.cycles,
        d.cps,
        d.cards_found,
        d.segmenter_used,
        d.last_result,
    ));
}
