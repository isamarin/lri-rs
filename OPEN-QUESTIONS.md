# Open questions — openfusion / L16

Handoff for the next session. Read [`FUSION.md`](FUSION.md) first — its log
carries every finding with a confidence tag; this file is just the open
worklist, most important first.

**Bench state:** real L16 on USB (fw 1.3.5.1), 61 real captures in
`.data-from-camera/raw/` (gitignored). Native engine pulled to
`vendor/light-l16/APKs/Firmware-1.3.5.1/libcp.so`. First open fused frame exists
(healthy module triple). Work is on branch `openfusion-extract`, committed
locally, **not pushed** (owner does not want this public yet).

That firmware directory (46 MB: `libcp.so`, `libcp.dylib`, both APKs) is
untracked **by decision, not by omission** — it is the redistribution exposure
called out in RE-LIGHT Phase 0.2, and the camera IP has sat with Samsung since
2021 (`PATENTS.md`). It is in the submodule's local `info/exclude` so a stray
`git add -A` cannot stage it. Publishing it is a deliberate call to make with a
clear head; until then it stays on this machine.

---

## 1. Mirror pose — ROOT CAUSE FOUND (2026-07-21)

> **Confidence: verified.** Reproduced on every capture tested (`L16_00001`,
> `00003`, `00020`, `00045`, `00078`) across focal 28 / 71 / 77 / 87 mm. Same
> four modules improper every time. Tool: `cargo run -p light --example pose_det`.

### The bug

```rust
let mut r = mat3_mul(reflection_matrix(n), p.r_cam);   // det = −1, ALWAYS
if p.flip_img_around_x { r = flip_x_mat(r); }          // ×(−1) → +1, only if true
```

`reflection_matrix` is a Householder reflection: **`det = −1` by construction**.
`r_cam` is a proper rotation (`+1`). Their product is therefore improper on
*every* module. `flip_x_mat` negates one row, multiplying the determinant by −1
again — so the composition yields a proper rotation **only when
`flip_img_around_x` is true**.

For modules where the file says `flip = false`, `R` is left improper. Always.

A camera extrinsic rotation lives in **SO(3)**: `det = +1` is not a convention,
it is what makes it a rotation. An improper pose matrix is never a valid state.
The mirror's effect on handedness is optically real, but it belongs to the
*image*, not to `R`.

### Measured (identical on all captures)

| module | mirror | path | flip | det(R) | NCC vs B4 (`L16_00003`) |
| --- | --- | --- | --- | --- | --- |
| B2 | Movable | mirror | **true** | +1 | 0.58 ok |
| B3 | Movable | mirror | **true** | +1 | 0.49 ok |
| C2 | Movable | mirror | **true** | +1 | negative ← still broken |
| C4 | Movable | mirror | **true** | +1 | negative ← still broken |
| B1 | Movable | mirror | **false** | **−1** | 0.11 |
| B5 | Movable | mirror | **false** | **−1** | 0.01 |
| C1 | Movable | mirror | **false** | **−1** | negative |
| C3 | Movable | mirror | **false** | **−1** | negative |
| A1–A5 | None | canon | — | +1 | — |
| B4, C5, C6 | Glued | canon | — | +1 | B4 is the reference, works |

`flip = true` ⟺ `det = +1`, 8 modules out of 8. The flag is a fixed per-module
hardware property (mirror mounting), constant across captures.

### Why it stayed hidden

B2 and B3 — the only two modules the flip was tuned on (`cff7a7d`, from
`L16_00078`, no camera) — both have `flip = true`. They come out proper and look
correct. **The bug is structurally invisible on exactly the tuning subset.**

### What to fix

`flip_img_around_x` is doing two jobs at once and they must be separated:

1. what the flag *means* — flip the image around its horizontal axis;
2. what it is *accidentally providing* — the parity of `R`.

`R` must be proper by construction for all modules; the image flip belongs at
warp time, driven by the flag. The RE target `0x1c7580` now has a precise
question instead of a vague one: **what restores parity in the real engine when
`flip_img_around_x` is false?** Look for a second parity-changing step (a
further reflection, an axis negation, or extraction of the rotation part) that
the port dropped.

### Guard now in place

`rotation_determinant()` (`lri-rs/src/mirror_pose.rs`, re-exported from the
crate root) plus an assertion in the orthogonality test. Note why the old test
missed this: `RᵀR = I` holds for `det = ±1` alike, so an orthogonality check
alone can never detect an improper rotation.

### Still open after the fix

C2 and C4 are **proper yet still correlate negatively**. Parity does not explain
them — they need the §1a angle/axis work, or their pointing is simply wrong (C is
150 mm tele; per the patents these modules aim at *different* parts of the scene,
so a wrong mirror angle sends them somewhere else entirely and NCC against B4
means little until pointing is right).

### Superseded hypotheses

- ~~"GLUED modules get no reflection / a different convention"~~ — **wrong**.
  B4, C5, C6 are `Glued`, take the `canonical` path, and are all proper. B4 is
  the reference camera and works. The canonical path is not implicated.
- ~~"tune the sign / flip / axis by search"~~ — the flip is not free (read from
  file), and the parity defect is unconditional, not per-module tuning.

---

## 1-old. Mirror pose — earlier framing (kept for context)

**Verification data** (capture `L16_00003`, focal 77, ref B4):

```
B4←B2  0.58   ok
B4←B3  0.49   ok
B4←B1  0.11   near zero      ← bug 1a
B4←B5  0.01   near zero      ← bug 1a
B4←C1..C4     negative       ← bug 1b, different failure mode
```

Near-zero and negative are **not the same symptom**. Misregistration trends to
zero. Systematically negative NCC is anti-correlation — a pose in the wrong
convention or a flipped handedness, not a near-miss. Treating C like a worse B
is why re-tuning has not converged.

The camera itself declares three mirror classes in the `.lri` protobuf
(`ltpb.GeometricCalibration.MirrorType`): `NONE=0`, `GLUED=1`, `MOVABLE=2` —
matching the patent family exactly (see [`PATENTS.md`](PATENTS.md) §2a: no
mirror / fixed mirror / movably hinged). The code never branches on this field
outside a test.

### 1a. B1/B5 — movable modules, sign or per-module data

Same class as B2/B3 (`MOVABLE`), so the model applies; something inside it is
wrong. `flip_img_around_x` was tuned blind on B2/B3 only, from archive file
`L16_00078`, with no camera (commit `cff7a7d`).

**Correction to the earlier framing:** `flip_img_around_x` is **not a free
parameter** — it is read from the file (`ms.flip_img_around_x`,
`mirror_pose.rs:129`). Do not "search" it; if B1/B5 need a different value than
the data supplies, the bug is in how the surrounding rotation is composed, not
in a boolean to be toggled.

Remaining genuinely free variables: angle sign and rotation-axis direction. The
axis is **further constrained by the patents** — "perpendicular to the Part B
optical axis and parallel to the plane of the front face" (PATENTS.md §2b), one
rotational DOF, nominal 45° mirror → 90° deflection. If `mirror_pose.rs` treats
the axis as free, pin it and one unknown collapses.

### 1b. C row — GLUED modules take a different code path entirely

`pick_focus_bundle_with_mirror` (`fusion.rs:214-247`) chooses:

1. `movable_mirror_bundle(...)` → pose computed from mirror system + Hall code,
   then the `0x1c79e0` post-process `t = -R * t_raw`;
2. **else** `pick_extrinsics_bundle(...)` → `canonical` R/t straight from the
   proto;
3. else nothing.

`MOVABLE` modules hit branch 1 (test `mirror_extrinsics_override_canonical_when_hall_present`).
`GLUED` modules have no movable bundle, so they fall to branch 2 and use
**factory-measured canonical extrinsics** — which already bake in the fixed
mirror's reflection.

So both classes do get a pose. The suspicion is that they get it in **different
conventions**, and the code mixes them in one fusion.

Note what "Verified correct" below actually says: the `world→cam, x_cam = R X + t`
convention was confirmed *because B2/B3 align* — i.e. validated **only on the
mirror-derived path**. It was never independently verified for `canonical`. If
canonical is stored as cam→world, or with `t` un-negated relative to the
post-processed mirror path, C-row poses are systematically inverted → negative
NCC.

**Rosetta-stone test (cheap, no RE):** `MOVABLE` modules carry *both*
representations — a `canonical` bundle *and* a movable-mirror bundle. Compute
both for the same module (B2 or B3, known-good) and compare. Any fixed
difference — transpose, negation, the `t = -R·t` step — is the exact conversion
that must be applied to the `GLUED` path. The camera hands us a labelled pair of
the same pose in two formats; use it before touching a disassembler.

**Second suspicion, independent of the above:**
`pick_extrinsics_bundle(&module.focus_calibrations)` takes **no focal length**,
while `pick_intrinsics_bundle(&module.focus_calibrations, shot_focal_mm)` does.
Extrinsics bundle selection therefore ignores the shot's focal length and may be
reading a bundle calibrated at the wrong focus distance. Check what it picks for
C modules at focal 77.

### Tools

- `LRI_DUMP_MIRROR=1` — angle / flip / axis / normal / n / cam_loc per module
- `LIGHT_FUSE_DEBUG=1` — per-pair NCC at infinity and at depth, with mirror type + baseline
- `LIGHT_FUSE_ONLY="B2,B3,B4"` — restrict fusion to a module subset

**Order of work:** confirm `mirror_type` per module in the sidecar first (one
command, settles whether C really is `GLUED`), then the Rosetta-stone
comparison, then 1a. Validate across B1, B5 **and** C together — never on B2/B3
alone, which is what produced this state.
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
- **What to recover:** the exact reflection/Rodrigues composition and the
  angle/axis sign. *Not* `flip_img_around_x` — that comes from the file, not
  from a decision in the engine (see §1a).
- **Only relevant to §1a.** For §1b the disassembler is the wrong tool: the
  question there is which convention `canonical` is stored in, and that is
  answerable from data alone via the Rosetta-stone test.

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
  ⚠️ Scope: B2/B3 are `MOVABLE`, so this validates the **mirror-derived path only**.
  The `canonical` extrinsics path (used by `GLUED` modules) is *assumed* to share
  this convention, never verified. See §1b.
- Matrix3x3F row-major read; K/R/t pulled from the right proto fields.
- Hall-code source: per-module `af_info.mirror_position` (not a global field).
- Mono C-row decoding (packed10bpp, sbro=(-1,-1)) — extract writes C1–C6 fine.
- Parallax sign in `homography_at_depth` — fixed (+, not −), with a sign test.
- Fusion preview debayer — fixed (box-average, was single-channel decimation).
- Module→focal map: A=28mm wide, B=70mm mid (always), C=150mm tele; ≤66→A+B, ≥71→B+C.
