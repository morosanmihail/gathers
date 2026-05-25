// card_ocr_live — webcam card scanner with Bevy UI.
//
// Usage: card_ocr_live [db_path]
//   db_path   Optional path to AllPrintings.db.  Defaults to
//             ~/.local/share/gathers/DB/AllPrintings.db.
//             If missing, card identification is skipped; extraction still runs.
//
// Layout:  70 % left = live webcam | 30 % right = identified-card log
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
use card_ocr::{extract_card_debug, identify_card_image};
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

const LOG_MAX: usize = 25;

// ── Resources ─────────────────────────────────────────────────────────────────

/// Latest raw RGB frame from the capture thread.
#[derive(Resource, Clone)]
struct LatestFrame(Arc<Mutex<Option<(Vec<u8>, u32, u32)>>>);

/// Receives formatted card strings from the OCR thread.
#[derive(Resource)]
struct OcrRx(Mutex<mpsc::Receiver<String>>);

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

    {
        let frame = shared_frame.clone();
        thread::spawn(move || capture_thread(frame));
    }

    let (ocr_tx, ocr_rx) = mpsc::channel::<String>();
    {
        let frame = shared_frame.clone();
        thread::spawn(move || ocr_thread(frame, ocr_tx, db_path));
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
        .insert_resource(CardLog::default())
        // Texture starts at 640×480; update_webcam_texture resizes on first frame.
        .insert_resource(CamSize(640, 480))
        .add_systems(Startup, setup)
        .add_systems(Update, (update_webcam_texture, poll_ocr, sync_log_text))
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
) {
    let retrieval = db_path.as_deref().and_then(|p| {
        MagicSQLiteRetrievalSystem::new(Some(p.to_string_lossy().into_owned()), None).ok()
    });

    // Single-threaded tokio executor: identify_card_image is async but the
    // underlying SQLite calls are sync, so a current-thread runtime is fine.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let mut last_entry: Option<String> = None;
    let mut last_status_print = Instant::now();
    let mut last_status = String::new();

    loop {
        // Clone frame data while holding lock as briefly as possible.
        let snapshot = frame.lock().ok().and_then(|g| g.clone());

        let Some((bytes, w, h)) = snapshot else {
            thread::sleep(Duration::from_millis(50));
            continue;
        };

        let Some(buf) = ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_raw(w, h, bytes) else {
            continue;
        };
        let dyn_img = DynamicImage::ImageRgb8(buf);

        let (extracted, status) = extract_card_debug(&dyn_img);

        // Print status at most once per 2 seconds, only when it changes.
        if last_status_print.elapsed() >= Duration::from_secs(2) || status.message != last_status {
            eprintln!("[extract] {}", status.message);
            last_status = status.message.clone();
            last_status_print = Instant::now();
        }

        let Some(extracted) = extracted else { continue };

        if let Some(ref r) = retrieval {
            match rt.block_on(identify_card_image(&extracted, r)) {
                Ok(Some(card)) => {
                    let entry = format!(
                        "{} ({} #{})",
                        card_name(&card),
                        card.get_set(),
                        card.get_collector_number()
                    );
                    if last_entry.as_deref() != Some(&entry) {
                        last_entry = Some(entry.clone());
                        let _ = tx.send(entry);
                    }
                }
                Ok(None) => {
                    // Card shape found but not matched in DB — reset dedup so
                    // the next successful match is always reported.
                    last_entry = None;
                }
                Err(e) => eprintln!("OCR error: {e}"),
            }
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

    // Create a CPU-writable RGBA texture for the webcam feed.
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
            // Centre the image so letterbox bars appear on all sides equally.
            root.spawn(Node {
                width: Val::Percent(70.),
                height: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|video| {
                // width fills the panel; height is derived from aspect_ratio so
                // the image never stretches.  Ratio is updated on first frame.
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

            // ── Right 30 %: card identification log ───────────────────────────
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
                panel.spawn((
                    Text::new("Identified Cards"),
                    TextFont { font_size: 18., ..default() },
                    TextColor(Color::srgb(0.9, 0.85, 0.4)),
                    Node {
                        margin: UiRect::bottom(Val::Px(10.)),
                        ..default()
                    },
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

/// Copy the latest RGB camera frame into the GPU texture each render frame.
/// Also updates the image node's aspect_ratio when camera dimensions change.
fn update_webcam_texture(
    latest: Res<LatestFrame>,
    mut images: ResMut<Assets<Image>>,
    tex: Res<WebcamTexture>,
    mut cam_node: Query<&mut Node, With<WebcamImageNode>>,
) {
    // try_lock: if capture/OCR thread holds the mutex this frame just skips.
    let Ok(guard) = latest.0.try_lock() else { return };
    let Some((rgb, w, h)) = guard.as_ref() else { return };
    let Some(image) = images.get_mut(&tex.0) else { return };

    let expected = (*w as usize) * (*h as usize) * 4;
    if image.data.len() != expected {
        image.texture_descriptor.size =
            Extent3d { width: *w, height: *h, depth_or_array_layers: 1 };
        image.data.resize(expected, 255);

        // Correct the aspect ratio now that we know real camera dimensions.
        if let Ok(mut node) = cam_node.get_single_mut() {
            node.aspect_ratio = Some(*w as f32 / *h as f32);
        }
    }

    // RGB → RGBA in-place
    for (dst, src) in image.data.chunks_exact_mut(4).zip(rgb.chunks_exact(3)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
        dst[3] = 255;
    }
}

/// Drain the OCR result channel and append to the log resource.
fn poll_ocr(rx: Res<OcrRx>, mut log: ResMut<CardLog>) {
    let Ok(rx) = rx.0.try_lock() else { return };
    while let Ok(entry) = rx.try_recv() {
        log.0.push(entry);
        if log.0.len() > LOG_MAX {
            log.0.remove(0);
        }
    }
}

/// Re-render log text whenever the log resource changes.
fn sync_log_text(log: Res<CardLog>, mut q: Query<&mut Text, With<LogText>>) {
    if !log.is_changed() {
        return;
    }
    if let Ok(mut t) = q.get_single_mut() {
        *t = Text::new(log.0.join("\n"));
    }
}
