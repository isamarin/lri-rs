# Lumen fusion — knowledge base

Living document for **humans and agents** building 16→1 combine on top of `lri-rs`.  
Add findings in small PRs; link code paths and protos; mark confidence.

**Maintained by:** isamarin × BLMK  
**Status:** research / incremental extraction  
**Last updated:** 2026-07-14

**L16 archive (submodule):** [`vendor/light-l16`](vendor/light-l16) → [github.com/isamarin/light-l16](https://github.com/isamarin/light-l16)

Clone with submodules: `git clone --recurse-submodules …` or `git submodule update --init`.

---

## Goal

Reconstruct the **single output image** the Light L16 / Lumen desktop app produced — not just 16 per-module DNGs.

Pipeline sketch:

```
.lri decode → per-module RAW (done)
           → shot metadata + calibration (in progress)
           → warp modules to common frame
           → depth map (ToF + disparity + focus)
           → exposure/colour match
           → blend (the hard part)
           → optional tone/HDR (view_preferences)
```

---

## What we already have (this fork)

| Layer | Status | Code |
| ----- | ------ | ---- |
| Block parse, zero-copy RAW | Done | `lri-rs/src/lib.rs`, `block.rs` |
| 10 bpp unpack, Bayer JPEG | Done | `unpack.rs`, `bayer_jpeg.rs` |
| Per-module DNG export | Done | `light/src/dng.rs`, `extract.rs` |
| Colour matrices (D65/F7) | Read | `block.rs` → `ColorInfo` |
| Sensor black/white | Read | `sensor_data` → `levels_for()` |
| Shot prefs (HDR, AWB, exposure) | Read | `ViewPreferences` |
| **Fusion metadata extract** | Done (v1) | `lri-rs/src/fusion.rs` → `LriFile.fusion` |
| GUI / gather shows fusion summary | Done | `light gather`, `api::LriSummary`, Lumen meta |

---

## Data in `.lri` relevant to fusion

Protobuf sources: [`lri-proto/proto/`](lri-proto/proto/)

### Per module (`LightHeader.module_calibration[]`)

| Proto field | Fusion role | Extracted? |
| ----------- | ----------- | ---------- |
| `geometry` (`GeometricCalibration`) | **Core** — intrinsics, extrinsics, mirror model | Yes (summary + K/R/t) |
| `geometry.per_focus_calibration[]` | Focus-dependent pose | Yes (list) |
| `geometry.distortion` | Lens model (polynomial / CRA) | Flags + coeffs when present |
| `geometry.extrinsics.moveable_mirror` | Mirror actuator + `MirrorSystem` | Flag + mirror pose |
| `vignetting` | Flat-field correction before blend | Flag only |
| `hot_pixel_map` / `dead_pixel_map` | Defect masking | Not yet |
| `color` | Per-module colour | Yes (`ColorInfo`) |

Key proto: [`geometric_calibration.proto`](lri-proto/proto/geometric_calibration.proto), [`mirror_system.proto`](lri-proto/proto/mirror_system.proto), [`distortion.proto`](lri-proto/proto/distortion.proto)

### Per shot (`LightHeader`)

| Field | Fusion role | Extracted? |
| ----- | ----------- | ---------- |
| `image_focal_length` | Pick `per_focus_calibration` bundle | Yes (`LriFile.focal_length`) |
| `image_reference_camera` | Reference module for alignment | Yes |
| `af_info` | Focus lock quality | Partial (`af_achieved`) |
| `tof_range` | Metric depth hint (metres?) | Yes |
| `device_calibration.tof` | Factory ToF linearization | Yes (offset/xtalk) |
| `imu_data[]` | Rolling shutter / motion | Sample counts |
| `gps_data` | Geo only (not geometry) | Lat/lon when present |
| `face_data` | Portrait ROI / weights? | Not yet |
| `view_preferences` | HDR tone, crop, orientation | Partial (no crop yet) |
| `proximity_sensors` | Near-field? | Not yet |
| `flash_data` | Flash modules | Not yet |

### Blocks not in LightHeader

| Block | Notes |
| ----- | ----- |
| `GPSData` (type 2) | Standalone GPS; parser skips protobuf parse on fast path — embedded `gps_data` in LightHeader is preferred |

---

## L16 hardware (why geometry is weird)

- **16 modules** in 3 rows (A1–A5, B1–B5, C1–C6); see [LRI.md](LRI.md) module grid.
- Many modules use a **movable mirror** (`MirrorType.MOVABLE`) — pose depends on focus/hall code, not fixed extrinsics.
- **ToF** sensor gives coarse depth; **multi-focus** modules give fine depth via disparity (hypothesis — verify against Lumen binary / papers).
- **Mono modules** (C row): likely used for depth / luminance, not colour.

Open questions — fill in when confirmed:

- [ ] Exact unit of `tof_range` (metres vs mm vs dioptres)
- [ ] How Lumen picks one `per_focus_calibration` entry for a given `image_focal_length`
- [ ] Whether `imu_data.frame_index` aligns to sensor row readout
- [ ] Role of `angle_optical_center_mapping` in final projection
- [ ] Output resolution and crop of the fused image vs single module

---

## Implementation roadmap

### Phase 0 — Inventory (current)

- [x] Document protos and gaps (this file)
- [x] Extract `FusionMeta` into `LriFile` (`geometry`, ToF, IMU, GPS)
- [x] Surface summary in `light gather` / API / Lumen

### Phase 1 — Calibration access

- [ ] Full `Distortion` + `VignettingCharacterization` structs
- [ ] `MirrorSystem` + `MirrorActuatorMapping` numeric extract
- [ ] Select focus bundle: match `image_focal_length` / hall code to `per_focus_calibration`
- [ ] Export fusion JSON sidecar next to DNG export

### Phase 2 — Geometric warp

- [ ] Project module RAW → common reference plane (likely `image_reference_camera`)
- [ ] Apply distortion + mirror model
- [ ] IMU-based row timing correction (if needed)

### Phase 3 — Depth + blend

- [ ] ToF-guided coarse depth map
- [ ] Refine with multi-module stereo / focus stack
- [ ] Per-pixel weights (aperture, vignette, distance to depth)
- [ ] Colour / exposure harmonization across modules

### Phase 4 — Output

- [ ] Fused 16-bit TIFF / DNG
- [ ] Match Lumen crop / orientation from `view_preferences`

---

## External references

| Resource | Notes |
| -------- | ----- |
| **[isamarin/light-l16](https://github.com/isamarin/light-l16)** | **Git submodule** at [`vendor/light-l16/`](vendor/light-l16/) — maintained L16 archive (fork of helloavo) |
| [helloavo/Light-L16-Archive](https://github.com/helloavo/Light-L16-Archive) | Upstream archive; isamarin fork is the working copy in this repo |
| [isamarin/lri-rs](https://github.com/isamarin/lri-rs) | This decoder / GUI / fusion R&D repo |
| [dllu/lri-rs](https://github.com/dllu/lri-rs) / [gennyble/lri-rs](https://github.com/gennyble/lri-rs) | Proto extraction basis |
| [LRI.md](LRI.md) | Container format |
| [bayer_jpeg.md](bayer_jpeg.md) | BJPG decode |
| Light L16 Discord (linked from archive README) | Owner reports, firmware |
| Wayback [light.co/camera](https://web.archive.org/web/20191222062257/https://light.co/camera) | Marketing / spec claims |

### `vendor/light-l16/` — subfolders to mine (agents: check these)

| Path | Contents | Fusion relevance |
| ---- | -------- | ---------------- |
| [`Lumen/`](vendor/light-l16/Lumen/) | Desktop app binaries | Reverse-engineer combine pipeline, shaders, strings |
| [`Hardware/`](vendor/light-l16/Hardware/) | Exploded view, sensor layout | Physical module positions vs `GeometricCalibration` |
| [`Guides/`](vendor/light-l16/Guides/) | L16 photography blog clone | Capture behaviour, marketing claims |
| [`APKs/`](vendor/light-l16/APKs/) | Camera / Gallery apps | On-device processing hints |
| [`L16 Lightroom Preset/`](vendor/light-l16/L16%20Lightroom%20Preset/) | Colour presets | Post-fusion look (not geometry) |

Firmware **1.3.5.1** — see archive README; release assets may be on [helloavo releases](https://github.com/helloavo/Light-L16-Archive/releases/tag/1.3.5.1) until mirrored in isamarin fork.

---

## Agent instructions

When working on fusion:

1. Read this file + [`LRI.md`](LRI.md) + relevant `.proto`.
2. Check **`vendor/light-l16/`** for hardware docs, Lumen binaries, guides — cite paths as `vendor/light-l16/...`.
3. Prefer **extract → unit test on real `.lri`** → document finding here.
4. Add a row to the tables above with **confidence**: `confirmed` / `likely` / `guess`.
5. Link PRs and file paths; keep prose short.
6. Do not block DNG/export work on fusion — land extractions incrementally.

### Entry template

```markdown
### YYYY-MM-DD — Short title

**Confidence:** likely  
**Source:** `path` or URL  
**Finding:** …  
**Implication for combine:** …  
**Follow-up:** …
```

---

## Log

### 2026-07-14 — Geometry lives in module_calibration

**Confidence:** confirmed  
**Source:** `lri-proto/proto/lightheader.proto`, `geometric_calibration.proto`  
**Finding:** Each `FactoryModuleCalibration` may carry full `GeometricCalibration` (intrinsics K, extrinsics R/t, per-focus bundles, movable mirror, distortion). Parser previously read only `color` from this message.  
**Implication:** Fusion does not need external calibration files — data is per capture in `.lri`.  
**Follow-up:** Implement focus-bundle selection; dump K/R/t in gather; compare across two `.lri` from same device.

### 2026-07-14 — No open-source combine implementation

**Confidence:** confirmed  
**Source:** This repo (`prism/`, `lri-study/`), helloavo archive README  
**Finding:** Community work stops at per-module extract; Lumen desktop did fusion closed-source.  
**Implication:** Green-field R&D; archive may help reverse-engineer, not copy-paste.  
**Follow-up:** Inspect archived Lumen app for algorithms / GPU shaders / log strings.

### 2026-07-14 — FusionMeta extraction landed

**Confidence:** confirmed  
**Source:** `lri-rs/src/fusion.rs`, `block.rs`, `light gather`  
**Finding:** Per-module `GeometricCalibration` (K/R/t per focus bundle, mirror type, distortion flags), shot `tof_range`, factory ToF cal, IMU sample counts, GPS fix now populate `LriFile.fusion`.  
**Implication:** First real `.lri` can validate whether all 16 modules ship geometry.  
**Follow-up:** Dump nearest focus bundle for `image_focal_length`; JSON sidecar export.

### 2026-07-14 — light-l16 archive as git submodule

**Confidence:** confirmed  
**Source:** `vendor/light-l16/`, [github.com/isamarin/light-l16](https://github.com/isamarin/light-l16)  
**Finding:** Fork of helloavo/Light-L16-Archive vendored at `vendor/light-l16/` (APKs, Guides, Hardware, Lumen desktop, Lightroom presets). Primary offline reference for RE and fusion archaeology.  
**Implication:** Agents and humans should search the submodule before the web; pin findings to paths under `vendor/light-l16/`.  
**Follow-up:** Mirror firmware 1.3.5.1 release in isamarin fork; strings/symbols pass on `vendor/light-l16/Lumen/`.

### 2026-07-14 — Purchase checklist (offline pipeline)

**Confidence:** confirmed (for this fork)  
**Finding:** Seller must demonstrate: boots → shoots → `.lri` on disk without Light cloud. All 16 modules present in file.  
**Implication:** Brick risk is activation/cloud, not decode — once you have `.lri`, this repo handles RAW.