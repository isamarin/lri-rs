# Anatomy of an LRI

LRI (Light Raw Image) is the proprietary RAW container used by the **Light L16**. A single capture is usually split across **10–11 blocks**; **40 blocks** have been observed on some files.

This document describes the format as implemented in this fork (`lri-rs` crate). Maintained by **isamarin × BLMK**. When the code and this file disagree, **trust the code** — and please fix the doc.

Protobuf definitions live in [`lri-proto/proto/`](lri-proto/proto/). Bayer JPEG details are in [`bayer_jpeg.md`](bayer_jpeg.md).

## File layout

An LRI file is a **concatenation of blocks**. There is no file-level header and no index. The parser reads blocks sequentially until the buffer is exhausted.

```
┌──────────┬──────────┬──────────┬─────
│ Block 0  │ Block 1  │ Block 2  │ ...
└──────────┴──────────┴──────────┴─────
```

Each block contains:

1. A fixed **32-byte binary header** (magic `LELR`)
2. A **protobuf message** at `message_offset`
3. Optionally **binary payloads** (RAW image data, maps, etc.) referenced by offsets inside the protobuf

The protobuf message does not necessarily fill the entire block. Image bytes often live **after** the protobuf, still inside the same block.

### Parsing algorithm (`LriFile::decode`)

Implemented in [`lri-rs/src/lib.rs`](lri-rs/src/lib.rs) and [`lri-rs/src/block.rs`](lri-rs/src/block.rs). Returns `Result<LriFile, LriError>` — truncated blocks and bad magic are errors, not panics.

1. Read `Header` from the front of the remaining buffer
2. Verify `block_length <= remaining bytes`
3. Slice `data[..block_length]` as one `Block` (includes the 32-byte header)
4. Call `block.extract_meaningful_data()` to accumulate images, colour profiles, camera info, and shot metadata
5. Advance: `data = data[block_length..]`
6. Repeat until `data` is empty
7. **Post-process**: for each `RawImage`, attach `sensor` from `CameraInfo` and `color` from `ColorInfo` by matching `CameraId`

All image payloads are **zero-copy** `&[u8]` slices into the original file buffer.

CLI: [`light`](light/) — `gather` (survey) and `extract` (parallel PNG export).

## Block header

32 bytes, little-endian:

| Offset | Size | Type | Field | Notes |
| ------ | ---- | ---- | ----- | ----- |
| 0 | 4 | bytes | signature | Must be `"LELR"` |
| 4 | 8 | u64 | `block_length` | Total size of this block **including** the 32-byte header |
| 12 | 8 | u64 | `message_offset` | Byte offset from **block start** to protobuf |
| 20 | 4 | u32 | `message_length` | Protobuf byte length |
| 24 | 1 | u8 | `message_type` | See table below |
| 25 | 7 | — | reserved | Not read by this parser |

### Message types

| `message_type` | Protobuf | Handled by parser |
| -------------- | -------- | ----------------- |
| 0 | [`LightHeader`](lri-proto/proto/lightheader.proto) | Yes — images, calibration, hw info, nested view prefs |
| 1 | [`ViewPreferences`](lri-proto/proto/view_preferences.proto) | Yes — exposure, gain, HDR, AWB, scene |
| 2 | [`GPSData`](lri-proto/proto/gps_data.proto) | Parsed, **not extracted** into `LriFile` |

```
Block byte layout (typical LightHeader block with embedded RAW):

0                    message_offset              message_offset + message_length
├─ 32-byte header ──┼─ (padding) ───────────────┼─ protobuf ──┼─ RAW payloads ─┤
│      "LELR"       │                           │ LightHeader │  image bytes   │
└───────────────────┴───────────────────────────┴─────────────┴────────────────┘
                                                      ↑
                                              data_offset points here
                                              (absolute offset from block start)
```

## LightHeader

The primary metadata container. Fields are spread across **multiple blocks** in practice — the parser merges them with `get_or_insert` / append semantics.

Full proto: [`lightheader.proto`](lri-proto/proto/lightheader.proto)

### What this fork extracts

| LightHeader field | Destination | Notes |
| ----------------- | ----------- | ----- |
| `hw_info.camera[]` | `Vec<CameraInfo>` | Maps `CameraId` → `SensorModel` |
| `module_calibration[]` | `Vec<ColorInfo>` | Per-camera colour profiles |
| `modules[]` | `Vec<RawImage>` | One entry per camera module with `sensor_data_surface` |
| `image_reference_camera` | `LriFile.image_reference_camera` | Viewfinder / reference camera |
| `device_fw_version` | `LriFile.firmware_version` | First non-empty wins |
| `image_focal_length` | `LriFile.focal_length` | |
| `af_info.focus_achieved` | `LriFile.af_achieved` | |
| `view_preferences` (nested) | Shot metadata via `extract_view()` | Same path as standalone ViewPreferences block |
| `sensor_data[]` | `LriFile.sensor_data` | Black/white levels via `levels_for(sensor)` |

### Not extracted (present in proto, ignored)

`image_unique_id_*`, `image_time_stamp`, `device_*`, `device_calibration`, `gold_cc`, `tof_range`, `proximity_sensors`, `flash_data`, `imu_data`, `gps_data` (also available as block type 2), `compatibility`, `face_data`, per-module `af_info`, geometry/vignetting/hot-pixel/dead-pixel maps inside `module_calibration`.

## L16 camera modules

The L16 exposes **16 camera modules** via [`CameraID`](lri-proto/proto/camera_id.proto):

| Group | IDs | Count |
| ----- | --- | ----- |
| A | A1–A5 | 5 |
| B | B1–B5 | 5 |
| C | C1–C6 | 6 |

Sensor types seen on L16 hardware (from [`hw_info`](lri-proto/proto/hw_info.proto) + [`sensor_type.proto`](lri-proto/proto/sensor_type.proto)):

| Proto enum | This fork | Role |
| ---------- | --------- | ---- |
| `SENSOR_AR1335` | `SensorModel::Ar1335` | Colour Bayer sensor |
| `SENSOR_AR1335_MONO` | `SensorModel::Ar1335Mono` | Monochrome / panchromatic |
| `SENSOR_AR835`, `SENSOR_IMX386`, `SENSOR_IMX386_MONO` | `unimplemented!()` | Not seen on L16 captures |

The `hw_info` → `CameraInfo` map can arrive in a **different block** than the `modules` image data. That is why `decode` links them in a final pass.

## RAW images (`CameraModule`)

Each [`CameraModule`](lri-proto/proto/camera_module.proto) with a `sensor_data_surface` becomes one `RawImage`.

### `Surface` fields

| Field | Meaning |
| ----- | ------- |
| `start` | Crop origin; always `(0, 0)` in observed L16 files |
| `size` | Image width (`x`) and height (`y`) |
| `format` | `RAW_BAYER_JPEG` (0) or `RAW_PACKED_10BPP` (7) |
| `row_stride` | Bytes per row for packed RAW; **0 for Bayer JPEG** |
| `data_offset` | Absolute byte offset from **block start** to image payload |
| `data_scale` | Optional; not used by this parser |

### Payload size

**Packed 10 bpp:**

```
payload_bytes = row_stride × height
```

The slice is `block[data_offset .. data_offset + payload_bytes]`.

> **Correction:** older notes said `row_stride × width`. The implementation uses **height** — one stride per row.

**Bayer JPEG:**

`row_stride` is 0. Total size comes from the [BJPG header](bayer_jpeg.md). The parser slices individual JPEG blobs but does not decode them in `lri-rs`.

### Per-module fields used

| Field | Maps to |
| ----- | ------- |
| `id` | `RawImage.camera` |
| `sensor_bayer_red_override` | `RawImage.sbro: (x, y)` — required; missing value panics |
| `sensor_data_surface` | dimensions, format, payload location |

Other module fields (`sensor_analog_gain`, `sensor_exposure`, flip flags, etc.) are present in proto but not read.

## `sensor_bayer_red_override` (CFA shift)

AR1335 colour modules use a BGGR CFA at the sensor. The override `(x, y)` indicates where **red** sits in the 2×2 Bayer tile. Implemented in `RawImage::cfa_string_ar1335()`:

| `sbro` | CFA string |
| ------ | ---------- |
| `(0, 0)` | BGGR |
| `(1, 0)` | GRBG |
| `(0, 1)` | GBRG |
| `(1, 1)` | RGGB |
| `(-1, -1)` | Monochrome / no CFA |

```
BGGR (base)              GRBG after override (1, 0)

B G B G                  G R G R
G R G R  ── x:1, y:0 ──> B G B G
B G B G                  G R G R
G R G R                  B G B G
```

The override likely compensates for in-camera rotation or mirroring. The same physical camera can report different `sbro` values across captures.

## Packed 10 bpp RAW

Format enum: `RAW_PACKED_10BPP` → `DataFormat::Packed10bpp`.

Unpacking: `RawImage::decode_pixels()` or `unpack()` (Packed10bpp only) → [`unpack::tenbit()`](lri-rs/src/unpack.rs).

- **10 bits per pixel**, 4 pixels packed into **5 bytes**
- Required input length: `ceil(count × 10 / 8)` bytes
- Packed bytes are read **from end to start** (no buffer copy)
- Main body: 5-byte chunks as big-endian `u64`, four 10-bit values extracted
- Remainder (< 5 bytes): little-endian, one pixel per 10-bit shift

Output is `Vec<u16>` with values in `0..=1023`.

### Levels

`LriFile::levels_for(sensor)` reads `sensor_data` black/white levels, falling back to **42 / 1023** (AR1335 defaults). Used by `light extract` when normalizing pixels.

## Bayer JPEG

See [`bayer_jpeg.md`](bayer_jpeg.md). Summary:

- `lri-rs`: parses BJPG header **and** decodes JPEG planes via `RawImage::decode_pixels()` ([`bayer_jpeg.rs`](lri-rs/src/bayer_jpeg.rs))
- `light extract`: exports both Packed10bpp and Bayer JPEG to PNG

## Colour calibration

Stored in `LightHeader.module_calibration` as [`FactoryModuleCalibration`](lri-proto/proto/lightheader.proto): one entry per camera, containing repeated [`ColorCalibration`](lri-proto/proto/color_calibration.proto).

### Extracted into `ColorInfo`

| Proto field | `ColorInfo` field | Usage |
| ----------- | ----------------- | ----- |
| `camera_id` | `camera` | Join key |
| `type` (IlluminantType) | `whitepoint` | A, D50, D65, D75, F2, F7, F11, TL84 |
| `forward_matrix` | `forward_matrix: [f32; 9]` | **Camera RGB → XYZ** (row-major 3×3) |
| `color_matrix` | `color_matrix: [f32; 9]` | Present in files; purpose unclear |
| `rg_ratio` | `rg` | Red/green ratio |
| `bg_ratio` | `bg` | Blue/green ratio |

Entries **without** `forward_matrix` are skipped.

`RawImage::daylight()` prefers illuminant **F7**, then **D65**.

### How `light extract` uses colour data

1. `decode_pixels()` — Packed10bpp or Bayer JPEG
2. Debayer using image `width`/`height` and CFA from `sbro`
3. Normalize with `levels_for(sensor)` black/white
4. Apply AWB gains from `ViewPreferences.awb_gains`
5. Multiply by `forward_matrix` (D65 profile) then Bruce XYZ→sRGB (D50 matrix)
6. sRGB gamma, write 8-bit PNG (parallel per camera via `rayon`)

`color_matrix`, `rg`/`bg`, and non-D65 illuminants are not used in the current export path.

## ViewPreferences

Can appear in two places:

1. **Standalone block** (`message_type = 1`) — only `extract_view()` runs
2. **Nested** in `LightHeader.view_preferences` (field 19)

Both paths merge into the same `ExtractedData` fields on `LriFile`:

| Field | `LriFile` field |
| ----- | --------------- |
| `image_integration_time_ns` | `image_integration_time: Duration` |
| `image_gain` | `image_gain` |
| `hdr_mode` | `hdr: HdrMode` |
| `scene_mode` | `scene: SceneMode` |
| `is_on_tripod` | `on_tripod` |
| `awb_mode` | `awb: AwbMode` (Auto, Daylight only mapped) |
| `awb_gains` | `awb_gain: AwbGain { r, gr, gb, b }` |

For scalar/enum fields the parser uses `get_or_insert` — **first non-empty value wins** across blocks.

Not extracted: `f_number`, `ev_offset`, `orientation`, `crop`, `aspect_ratio`, `view_preset`, extra AWB modes (Shade, Cloudy, Tungsten, …).

## GPS

`GPSData` may appear as `message_type = 2` or embedded in `LightHeader.gps_data`. The parser can deserialize it but **`extract_meaningful_data` returns early** for GPS blocks — coordinates never reach `LriFile`.

## Implementation gaps (this fork)

| Area | Status |
| ---- | ------ |
| Packed10bpp read + unpack | Working |
| Bayer JPEG decode → pixels | Working (`decode_pixels`) |
| Colour calibration in API | Read; used by `light extract` for D65 only |
| `sensor_data` black/white levels | Exported; `levels_for()` |
| GPS, IMU, faces, geometry, vignetting | Ignored |
| DNG export | Not implemented |
| 10-bit Bayer JPEG reconstruction | 8-bit JPEG promoted with `<< 2`; summing planes not implemented |

## Related files

| File | Role |
| ---- | ---- |
| [`lri-rs/src/lib.rs`](lri-rs/src/lib.rs) | `LriFile`, `RawImage`, public API |
| [`lri-rs/src/block.rs`](lri-rs/src/block.rs) | Block parsing and extraction |
| [`lri-rs/src/unpack.rs`](lri-rs/src/unpack.rs) | 10 bpp unpack |
| [`lri-rs/src/types.rs`](lri-rs/src/types.rs) | Proto → Rust enum mapping |
| [`light/`](light/) | CLI: `gather`, `extract` |

## External resources

- [helloavo/Light-L16-Archive](https://github.com/helloavo/Light-L16-Archive) — firmware, root, L16 tooling
- [dllu/lri-rs](https://github.com/dllu/lri-rs) — original protobuf extraction from Lumen (basis for `lri-proto`)