# card_ocr

Identifies Magic: The Gathering cards from images using OCR and optional computer-vision segmentation.

## Binaries

### `card_ocr` — identify a single image

```bash
cargo run --bin card_ocr -- <image_path> [db_path]
```

- `image_path` — path to the card photo (JPEG, PNG, etc.)
- `db_path` — optional path to `AllPrintings.db`; defaults to `~/.local/share/gathers/DB/AllPrintings.db`

### `card_ocr_live` — live webcam scanner

```bash
cargo run --bin card_ocr_live -- [db_path]
```

Opens the default camera. Left panel shows the live feed; right panel shows identified cards and debug info.

## Segmentation features

By default only OCR is run on the raw image. Two optional segmenters can isolate the card before OCR:

| Feature | Segmenter | Requirement |
|---|---|---|
| `opencv-seg` | OpenCV contour detection | `opencv` system library |
| `onnx-seg` | YOLOv8 ONNX detection | `onnxruntime` system library + model file |

Enable one or both:

```bash
cargo run --bin card_ocr --features opencv-seg -- image.jpg
cargo run --bin card_ocr --features onnx-seg -- image.jpg
cargo run --bin card_ocr --features opencv-seg,onnx-seg -- image.jpg
```

When multiple features are enabled, OpenCV is tried first, then ONNX. The first successful extraction is used.

### OpenCV (Arch Linux)

```bash
sudo pacman -S opencv
```

Build with:

```bash
cargo build --features opencv-seg
```

### ONNX / YOLOv8

**Install runtime (Arch Linux):**

```bash
sudo pacman -S onnxruntime
```

Set link flags so the crate uses the system library instead of bundling its own:

```bash
export ORT_PREFER_DYNAMIC_LINK=1
export ORT_LIB_LOCATION=/usr/lib
```

**Get the model:**

Export YOLOv8n from Ultralytics (requires Python + `ultralytics` package):

```bash
pip install ultralytics
python3 -c "from ultralytics import YOLO; YOLO('yolov8n.pt').export(format='onnx', opset=12)"
```

This downloads `yolov8n.pt` and writes `yolov8n.onnx` in the current directory.

Place the model somewhere and point to it:

```bash
export YOLO_MODEL_PATH=/path/to/yolov8n.onnx
```

Then run:

```bash
ORT_PREFER_DYNAMIC_LINK=1 ORT_LIB_LOCATION=/usr/lib \
  cargo run --bin card_ocr --features onnx-seg -- image.jpg
```

A pre-exported model is stored in `data/yolov8n.onnx` relative to the workspace root.

## Environment variables

| Variable | Description |
|---|---|
| `YOLO_MODEL_PATH` | Path to YOLOv8 ONNX model (required for `onnx-seg`) |
| `ORT_PREFER_DYNAMIC_LINK` | Set to `1` to use system ONNX Runtime instead of bundled static build |
| `ORT_LIB_LOCATION` | Directory containing `libonnxruntime.so` (e.g. `/usr/lib`) |
| `MTG_DB_PATH` | Override default `AllPrintings.db` path |
