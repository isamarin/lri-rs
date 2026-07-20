# Open questions — openfusion / L16

Handoff for the next session (Grok). Read [`FUSION.md`](FUSION.md) first — its
log carries every finding with a confidence tag; this file is just the open
worklist, most important first.

**Bench state:** real L16 on USB (fw 1.3.5.1), 61 real captures in
`.data-from-camera/raw/` (gitignored). Native engine pulled to
`vendor/light-l16/APKs/Firmware-1.3.5.1/libcp.so`. First open fused frame exists
(healthy module triple). Work is on branch `openfusion-extract`, committed
locally, **not pushed** (owner does not want this public yet).

---

## 1. Mirror pose for non-B2/B3 movable modules — blocker #1

`mirror_pose.rs` gives correct R/t for B2/B3 but wrong for B1, B5 and the whole
C row. Root cause: Grok tuned `flip_img_around_x` blind on B2/B3 only, from the
archive file `L16_00078`, with no camera (commit `cff7a7d`). Everything else in
the geometry is already verified correct (see "Verified" below) — the bug is
localized here.

- **Verification data** (capture `L16_00003`, focal 77, ref B4):
  - healthy: B4←B2 NCC 0.58, B4←B3 0.49
  - broken: B4←B1 0.11, B4←B5 0.01, B4←C1..C4 negative
- **Tools:**
  - `LRI_DUMP_MIRROR=1` — dumps angle / flip / axis / normal / n / cam_loc per module
  - `LIGHT_FUSE_DEBUG=1` — per-pair NCC at infinity and at depth, with mirror type + baseline
  - `LIGHT_FUSE_ONLY="B2,B3,B4"` — restrict fusion to a module subset
- **Approach:** systematically flip one variable at a time (angle sign / rotation-axis
  direction / the `flip_img_around_x` condition) using per-pair NCC as the oracle.
  Do NOT re-tune on B2/B3 alone — validate across B1, B5, and C modules together.
### RE fallback — if the empirical flip/sign search stalls

Do empirics first (above). Only reverse the engine if the sign/flip won't yield.

- **Target:** the mirror function in the ARM `libcp.so` at **`0x1c7580`** (and
  `0x1c79e0` for the translation post-process) — `mirror_pose.rs` claims to port
  exactly these. Grok already located the addresses, so we have an anchor.
- **No symbols to lean on:** both binaries are stripped inside. `nm` shows only
  the public CIAPI (`DirectRenderer`, `RendererBase`); mirror math is unnamed
  internal in *both* the `.so` and the desktop `.dylib`. So RE is address-driven,
  not name-driven — the desktop build is NOT easier for this.
- **Nothing required to start:** system `objdump` already disassembles the ARM
  `.so` (`objdump -d --start-address=0x1c7580 --stop-address=0x1c78c0 libcp.so`).
  Raw asm is readable now; the math is float-SIMD (reflection, Rodrigues, flip).
- **Optional, for readable C:** `brew install radare2` (headless `pdg`/r2ghidra)
  or `brew install openjdk` + Ghidra. Speeds up formula comparison vs asm; not a
  blocker.
- **Both engines are extracted** to `vendor/light-l16/APKs/Firmware-1.3.5.1/`:
  `libcp.so` (ARM — the mirror target, has the address) and `libcp.dylib`
  (desktop — use for public API / pipeline order, not mirror formulas).
- **What to recover:** confirm the exact reflection/Rodrigues composition, the
  `flip_img_around_x` condition, and the angle/axis sign — the three variables
  the empirical search is toggling.

## 2. Per-pixel depth (SGM) — replace the single plane

Architecture is independently confirmed by patent **US 9,563,033 B2** (depth
from stereo/parallax) — see [`PATENTS.md`](PATENTS.md). It backs the SGM path but
contains no engine mechanics.

Residual softness in the fused frame is the single fronto-parallel plane limit.
The engine does a dense per-pixel `WarpField` (symbols `DepthToDisparity`;
depth is **mm along the optical axis, inverse-range**). Replace `plane_sweep`
(one global Z) with per-pixel depth → warp field. Needs a wide-baseline pair
(`libcp` warns `Baseline too small` — B4↔B1 is too close; pick widely separated).

## 3. `tof:0.00` on every capture

ToF reads 0.00 on all captures (wide and tele). Either not written to `.lri`,
parsed wrong, or genuinely unused (depth comes from SGM, ToF is only a seed).
Confirm on tele captures; decide whether depth seeding needs it.

## 4. Exact reference CameraId within a group

Reference = "widest module that fired" (A-row when wide, B-row when tele —
confirmed `reference camera: B4` at focal 77). Which *specific* module within
the group is picked is not nailed down.

## 5. openfusion → submodule

The fusion geometry core (`warp` + `stereo`, nalgebra-only) is extracted into the
`openfusion/` crate, currently a plain folder in the monorepo (builds via
path-dep). To split into its own repo + submodule when publishing is desired:

```bash
cd openfusion
gh repo create isamarin/openfusion --public --source=. --remote=origin --push
cd ..
git rm -r --cached openfusion 2>/dev/null; rm -rf openfusion/.git
git submodule add https://github.com/isamarin/openfusion openfusion
git commit -m "extract openfusion fusion-geometry crate as submodule"
```

## 6. Dataset hygiene

61 captures in `.data-from-camera/raw/` (gitignored). Before any public release,
strip GPS blocks and screen recognizable locations.

---

## Verified correct — do not re-litigate

- R/t convention (world→cam, `x_cam = R X + t`) — empirically confirmed (B2/B3 align).
- Matrix3x3F row-major read; K/R/t pulled from the right proto fields.
- Hall-code source: per-module `af_info.mirror_position` (not a global field).
- Mono C-row decoding (packed10bpp, sbro=(-1,-1)) — extract writes C1–C6 fine.
- Parallax sign in `homography_at_depth` — fixed (+, not −), with a sign test.
- Fusion preview debayer — fixed (box-average, was single-channel decimation).
- Module→focal map: A=28mm wide, B=70mm mid (always), C=150mm tele; ≤66→A+B, ≥71→B+C.
