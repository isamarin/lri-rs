# lri-rs

Rust workspace for **Light L16** `.lri` (Light Raw Image) files — parse, survey, export per-camera RAW, and research the Lumen 16→1 fusion pipeline.

Fork maintained by **isamarin × BLMK**. Version: **CalVer** (`YYYY.M.D`) — see `VERSION` and `./scripts/calver`.

## Quick start

```bash
# Optimized release build (LTO + native CPU flags on Apple Silicon)
make release

# Survey a folder of captures (includes fusion metadata summary)
./target/release/light gather /path/to/photos/

# Extract all camera modules to DNG (parallel, mmap-backed)
./target/release/light extract photo.lri ./output/ --jobs 8
```

Install globally:

```bash
make install   # → ~/.cargo/bin/light
```

### Desktop GUI (Tauri 2)

```bash
make lumen-release
./target/release/lumen
```

Drag-drop `.lri`, camera grid with parallel thumbnails, DNG export with progress bar. Session cache avoids re-reading the file on every action.

For live reload during UI work, install the Tauri CLI once (`cargo install tauri-cli`) and run `cargo tauri dev` from `lumen/src-tauri`.

## `light` CLI

| Command | Description |
| ------- | ----------- |
| `light gather <dir>` | Metadata + fusion summary for every `.lri` (parallel scan) |
| `light extract <lri> <out> [--jobs N]` | Per-camera DNG export; `LIGHT_JOBS` env or P-core count on macOS |

`gather` appends fusion hints per file, e.g. `fus geo:16/16 mir:12 tof:1.23 imu:4 gps`.

Replaces the older `prism` and `lri-study` binaries (still in repo, no longer in workspace).

## Workspace

| Crate | Role |
| ----- | ---- |
| **lri-rs** | Library — `LriFile::decode()`, `RawImage::decode_pixels()`, `LriFile.fusion` |
| **lri-proto** | Protobuf types ([dllu/lri-rs](https://github.com/dllu/lri-rs) / Lumen) |
| **light** | CLI + shared lib (DNG, thumbnails, session cache) |
| **lumen** | Tauri 2 desktop GUI |

## Documentation

- [LRI.md](LRI.md) — block format, cameras, colour calibration
- [bayer_jpeg.md](bayer_jpeg.md) — BJPG container
- [FUSION.md](FUSION.md) — Lumen combine research log (geometry, depth, blend) — **living doc for humans and agents**
- [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) — open worklist, most important first; start here to pick something up
- [PATENTS.md](PATENTS.md) — what Light's patents do and do not disclose. **Read before attacking geometry:** they give *structure* (three mirror classes, hinge-axis constraints) and never mechanics, and knowing which is which saves days
- [RE-LIGHT.md](RE-LIGHT.md) — the plan: phases, gates, and where this is meant to end up

## Library example

```rust
let data = std::fs::read("photo.lri")?;
let lri = lri_rs::LriFile::decode(&data)?;

for img in lri.images() {
    let pixels = img.decode_pixels()?; // Packed10bpp + Bayer JPEG
    let (black, white) = lri.levels_for(img.sensor);
}

// Fusion pipeline inputs (geometry, ToF, IMU, GPS)
let fusion = &lri.fusion;
println!("geometry modules: {}", fusion.geometry_module_count());
```

Via `light` session API (mmap + cached decode):

```rust
let session = light::session::LriSession::open("photo.lri")?;
session.with_lri(|lri| { /* ... */ })?;
```

## Versioning (CalVer)

| File / tool | Role |
| ----------- | ---- |
| `VERSION` | Single source of truth (`2026.7.14`) |
| `./scripts/calver` | `show`, `sync`, `check`, `bump`, `bump-micro` |
| `make version-bump` | Set today's UTC date and sync `Cargo.toml` + `tauri.conf.json` |

Same-day rebuilds use semver pre-release: `2026.7.14-dev.1`.

Release tag: `git tag v2026.7.14 && git push --tags` → GitHub Actions builds binaries.

## CI

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) on push/PR:

- CalVer consistency check
- `cargo test --workspace`
- Release build: `light` on Linux, `light` + `lumen` on macOS

[`.github/workflows/release.yml`](.github/workflows/release.yml) — artifacts on version tags.

Local checks:

```bash
make version-check
cargo test --workspace
make bench    # tenbit unpack benchmark
```

## Apple Silicon tuning

| Setting | Location |
| ------- | -------- |
| `target-cpu=native` | [`.cargo/config.toml`](.cargo/config.toml) |
| Fat LTO, 1 codegen unit | `[profile.release]` in root `Cargo.toml` |
| `release-fast` profile | Thin LTO for quicker iteration (`make release-fast`) |
| P-core thread count | `light/src/threads.rs` — `sysctl hw.perflevel0.physicalcpu` |
| Zero-copy block parse | `lri-rs` mmap / slices into input buffer |
| 10 bpp unpack | 8× unrolled (`lri-rs/src/unpack.rs`) |
| Fast grid thumbnails | Single JPEG plane + parallel batch (`light/src/thumbnail.rs`) |
| Session cache | `LriSession` — one decode per open file (`light/src/session.rs`) |
| Parallel DNG export | `rayon` in `light extract` |

## What works

| Feature | Status |
| ------- | ------ |
| Block parse with error handling | Yes |
| Packed 10 bpp unpack | Yes |
| Bayer JPEG decode → pixels (`zune-jpeg` 0.5) | Yes |
| DNG export (both RAW formats) | Yes |
| GUI thumbnails + drag-drop + export progress | Yes (`lumen`) |
| `sensor_data` black/white levels | Yes (`levels_for`) |
| Fusion metadata (geometry K/R/t, ToF, IMU, GPS) | Partial — [FUSION.md](FUSION.md) |
| 16→1 Lumen combine | Not yet — research in [FUSION.md](FUSION.md) |

## Resources

- [`vendor/light-l16/`](vendor/light-l16/) — git submodule, [isamarin/light-l16](https://github.com/isamarin/light-l16) (L16 archive: firmware notes, Lumen app, hardware, guides)
- [FUSION.md](FUSION.md) — submodule paths and fusion research log

Clone:

```bash
git clone --recurse-submodules https://github.com/isamarin/lri-rs.git
# or after clone:
git submodule update --init
```

## Credits

- Original parser & docs — [gennyble](https://github.com/nyble) / [dllu/lri-rs](https://github.com/dllu/lri-rs)
- This fork — **isamarin × BLMK**

## Licensing

The workspace as a whole is **AGPL-3.0-or-later** — [LICENSE](LICENSE).
Per-component notices and the reasoning: [COPYRIGHT](COPYRIGHT).

- **Fork changes — AGPL-3.0-or-later, isamarin × BLMK**
- Upstream crates (`lri-rs`, `light`, …) — ISC, gennyble \<gen@nyble.dev\>
- `lri-proto` — MIT, Daniel Lawrence Lu

Use it, fork it, run it, charge for it. The one thing you cannot do is close it:
ship a modified version — or a network service built on it (§13) — and the
source goes with it. This camera was abandoned by its maker and kept alive by
the people who owned one; what they rebuilt should not be enclosed by anyone,
us included.

Note for contributors: AGPL code cannot be merged back into the ISC upstream, so
anything offered to `gennyble/lri-rs` has to be limited to changes we can also
release under ISC.

`vendor/light-l16/` is outside all of this: archived Light L16 material
(firmware, stock app, docs) preserved because the originals are disappearing.
Rights there remain with the original holders.