# lri-rs

Rust workspace for **Light L16** `.lri` (Light Raw Image) files — parse, survey, and export per-camera RAW.

## Quick start

```bash
# Optimized release build (LTO + native CPU flags on Apple Silicon)
make release

# Survey a folder of captures
./target/release/light gather /path/to/photos/

# Extract all camera modules to PNG (parallel)
./target/release/light extract photo.lri ./output/ --jobs 8
```

Install globally:

```bash
make install   # → ~/.cargo/bin/light
```

## `light` CLI

| Command | Description |
| ------- | ----------- |
| `light gather <dir>` | Metadata report for every `.lri` in a directory |
| `light extract <lri> <out> [--jobs N]` | Per-camera PNG export (rayon-parallel) |

Replaces the older `prism` and `lri-study` binaries (still in repo, no longer in workspace).

## Workspace

| Crate | Role |
| ----- | ---- |
| **lri-rs** | Library — `LriFile::decode()`, `RawImage::decode_pixels()` |
| **lri-proto** | Protobuf types ([dllu/lri-rs](https://github.com/dllu/lri-rs) / Lumen) |
| **light** | CLI tool |

## Documentation

- [LRI.md](LRI.md) — block format, cameras, colour calibration
- [bayer_jpeg.md](bayer_jpeg.md) — BJPG container

## Library example

```rust
let data = std::fs::read("photo.lri")?;
let lri = lri_rs::LriFile::decode(&data)?;

for img in lri.images() {
    let pixels = img.decode_pixels()?; // Packed10bpp + Bayer JPEG
    let (black, white) = lri.levels_for(img.sensor);
}
```

## Apple Silicon tuning

This fork is set up for fast native builds on M-series Macs:

| Setting | Location |
| ------- | -------- |
| `target-cpu=native` | [`.cargo/config.toml`](.cargo/config.toml) |
| Fat LTO, 1 codegen unit | `[profile.release]` in root `Cargo.toml` |
| `release-fast` profile | Thin LTO for quicker iteration (`make release-fast`) |
| Zero-copy block parse | `lri-rs` keeps slices into input buffer |
| Allocation-free 10 bpp unpack | reads packed RAW backwards in-place |
| Parallel PNG export | `rayon` in `light extract` |

Benchmark 10 bpp unpack:

```bash
make bench
```

## What works (v0.2)

| Feature | Status |
| ------- | ------ |
| Block parse with error handling | Yes |
| Packed 10 bpp unpack | Yes |
| Bayer JPEG decode → pixels | Yes |
| PNG export (both RAW formats) | Yes |
| `sensor_data` black/white levels | Yes (via `levels_for`) |
| GPS / IMU / geometry | Proto only |
| DNG export | Planned |

## Resources

- [helloavo/Light-L16-Archive](https://github.com/helloavo/Light-L16-Archive) — firmware, root, archived L16 tooling

## Licensing

- `lri-proto` — MIT, Daniel Lawrence Lu
- Everything else — ISC, gennyble \<gen@nyble.dev\>