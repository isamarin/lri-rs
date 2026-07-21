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
untracked **by decision, not by omission** — redistributing a vendor binary is a
different kind of exposure from publishing a clean-room parser, and the camera IP
has sat with Samsung since 2021 (`PATENTS.md`). It is in the submodule's local
`info/exclude` so a stray `git add -A` cannot stage it. Publishing it is a
deliberate call to make with a clear head; until then it stays on this machine.

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

### Half fixed, 2026-07-21 — and the other half is now visible

`flip_img_around_x` was doing two jobs: the image flip it names, and the parity
of `R` it supplied by accident. The parity job is now unconditional —
`compute_mirror_rt_raw` applies `flip_x_mat` for every module, so `R` is proper
by construction and does not depend on the flag. The flag survives as
`MirrorExtrinsics.image_flip_x`, **carried but not consumed**: nothing applies an
image flip at warp time yet.

`det = +1` on all 16 modules across `00001`, `00003`, `00020`, `00045`, `00078`.
The `mirror_extrinsics_produce_proper_rotation` test is green, and a second test
pins parity for *both* settings of the flag so it cannot creep back into `R`.

**The change is provably isolated.** Per-module NCC moved on exactly B1, B5, C1,
C3 — the four `flip = false` modules — and on nothing else, to the last digit.
The twelve working modules are untouched.

#### What the numbers say (`light validate`, per-module NCC vs reference)

| module | 00001 28/A1 | 00003 77/B4 | 00020 28/A1 | 00045 71/B4 | 00078 87/A1 |
| --- | --- | --- | --- | --- | --- |
| B1 | +0.101 → +0.112 | +0.054 → +0.068 | −0.040 → −0.059 | +0.076 → +0.076 | **+0.348 → −0.262** |
| B5 | −0.028 → +0.028 | +0.003 → +0.059 | +0.041 → −0.028 | −0.001 → +0.094 | **+0.344 → −0.258** |
| C1 | — | −0.061 → **+0.068** | — | −0.022 → **+0.023** | — |
| C3 | — | −0.059 → **+0.012** | — | +0.014 → +0.002 | — |

C1 is the clean confirmation: negative → positive on both captures where it
fires, which is the parity signature and nothing else.

#### The counterexample, stated rather than buried

On `00078` B1 and B5 went from **+0.35 to −0.26**. That capture is not
dismissible: all five B modules there share the same 16.5 % overlap with the A1
reference, and B2/B3/B4 score +0.44/+0.34/+0.57 under identical conditions. It
is the best-conditioned measurement we have, and it is the one that got worse.

Read it as the diagnosis it is: **B1/B5 are still in a mirrored frame after `R`
was made proper.** A strong negative NCC is the mirrored-frame signature, not
noise. Before the fix, the improper `R` was smuggling the image flip through the
homography — the algebra was wrong but the warp came out right, on this capture.
Making `R` proper removed a flip the warp still needs, and the elsewhere-mushy
numbers (|NCC| < 0.1 on the 28 mm captures) were too weak to show it.

So the fix is correct and incomplete. `det = +1` is an invariant, not a
preference — an improper pose is never an acceptable state, and a number that
looked better while the model was wrong was a number worth losing.

#### Next, and the fork to be careful about

The remaining difference between {B2, B3, C2, C4} and {B1, B5, C1, C3} is now
*only the flag itself*, since it no longer enters `R`. The obvious hypothesis is
that `flip_img_around_x` describes how the sensor image already arrives, so the
modules where it is **false** are the ones needing the flip applied downstream.

That has a falsifiable prediction, which is why it was worth one run rather than a
search: applying an image flip at warp time to the four `flip = false` modules
should send B1/B5 on `00078` back to ≈ +0.3 **without moving B2/B3/C2/C4 at all**.

#### Run made 2026-07-21 — prediction held, and split the problem in two

`LRI_WARP_FLIP=flag_false|flag_true` in `light validate` right-composes a
source-coordinate flip (`y ↦ (h−1) − y`) onto the homography for the selected
modules. Default off; no shipped behaviour depends on it.

The prediction hit exactly: B1 → **+0.349** and B5 → **+0.315** on `00078`, and
B2/B3/C2/C4 did not move by a single digit. Running the *inverse* (flip the
`flag = true` modules) breaks B2/B3 just as hard — +0.44 → −0.15 and +0.34 →
−0.26 on the same capture.

| | flag | parity only | + flip flag=false | + flip flag=true |
| --- | --- | --- | --- | --- |
| B1 `00078` | false | −0.262 | **+0.349** | −0.262 |
| B5 `00078` | false | −0.258 | **+0.315** | −0.258 |
| B2 `00078` | true | +0.442 | +0.442 | **−0.151** |
| B3 `00078` | true | +0.345 | +0.345 | **−0.258** |

> **Retracted the same day.** This section originally read "for the B row this is
> settled: the image needs flipping exactly when `flip_img_around_x` is false."
> That was wrong, and wrong in the project's signature way — it was concluded
> from captures that do not disagree with each other. See §1c: the rule holds
> against an A-row reference and fails against B4, with identical flags in both
> files. Kept here because the measurements are sound; only the conclusion drawn
> from them was not.

#### Why this is not yet wired in

The C row does not follow the same rule, and the C numbers are too weak to
establish any rule at all. C2 — one of the two "proper yet negative, unexplained"
modules — turns positive under `flag_true` (−0.082 → +0.090 on `00003`, −0.055 →
+0.038 on `00045`), its first movement ever. But C1 and C3 get worse under
`flag_false` on `00003` and C3 gets *better* on `00045`: the sign is not stable
across captures, at |NCC| < 0.07 throughout.

That is the instrument failing, not a subtle law. C modules overlap the B4
reference by only ~20 % of the canvas and, per `PATENTS.md`, aim at a different
part of the scene entirely — `light grid` shows it plainly. NCC against B4 cannot
adjudicate a flip hypothesis under those conditions.

So: **do not encode "flip when flag is false" yet**, and above all do not encode
a per-row exception to it. Fitting a rule to the C row on |0.06| numbers from one
camera is precisely the mistake that produced the original defect. What the B row
establishes is that the *mechanism* is right; the open part is whether one rule
covers all eight mirror modules.

#### Next

1. Fix C pointing first (§1a). Until a C module lands on the same scene content
   as the reference, its NCC is not a measurement.
2. Then re-run the same two-config experiment on the C row. If one rule covers
   all eight, wire it in and delete the env switch.
3. If the rows genuinely disagree, that is the sharpened RE question for
   `0x1c7580`: *what does the engine key the image flip on, given parity is
   already handled and the flag alone does not cover both rows?*

### Guard now in place

`rotation_determinant()` (`lri-rs/src/mirror_pose.rs`, re-exported from the
crate root) plus an assertion in the orthogonality test. Note why the old test
missed this: `RᵀR = I` holds for `det = ±1` alike, so an orthogonality check
alone can never detect an improper rotation.

### Still open after the fix

C2 and C4 are **proper yet still correlate negatively** (−0.082 / −0.007 on
`00003`, −0.055 / −0.011 on `00045`), and the parity fix did not touch them —
they already had `flip = true`. Parity does not explain them: they need the §1a
angle/axis work, or their pointing is simply wrong (C is 150 mm tele; per the
patents these modules aim at *different* parts of the scene, so a wrong mirror
angle sends them somewhere else entirely and NCC against B4 means little until
pointing is right).

Supporting evidence for the pointing reading, cheap to reproduce:
`light grid .data-from-camera/raw/L16_00003.lri <out>` renders all sixteen
modules as a contact sheet, and the C2/C4 cells visibly frame a different part
of the scene than the B row. Worth looking at before spending more on NCC.

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

## 1c. C pointing — investigated 2026-07-21, and it is not pointing

Went looking for the C row's aim error. It is not there. Four suspects were
eliminated by measurement, one new instrument came out of it, and the flip rule
from §1 was refuted.

New tool: `cargo run -p light --example mirror_aim -- <lri>` — mirror angle vs
the range the calibration declares, optical axis vs the reference, K per module,
and the parallax an infinity homography is throwing away.

### Eliminated

- **Hall→angle mapping.** All eight mirror modules land inside their own declared
  `mirror_angle_range` on `00003`, with 0.9–3.4° of margin. Not a tuning matter:
  an out-of-range angle would have meant the camera says the mirror cannot be
  there. It never happens.
- **Pointing itself.** Every module's optical axis (`Rᵀ·ẑ`) sits within 6° of the
  reference — A row 0.7–1.0°, B row 0.3–1.7°, C row 0.6–5.7°. Nothing is aimed
  away.
- **~~"C modules aim at different parts of the scene" (from the patents)~~** —
  **not supported by our data.** The poses put the C row on the same scene as
  everything else. What looks like different framing on the contact sheet is a
  150 mm crop of the same view, not a different view. The patents describe what
  the design *can* do; this capture does not do it.
- **Intrinsics and focus-plane selection.** C1–C4 and C5/C6 draw `fx ≈ 18 450`,
  comparable principal points, and identical focus distances (int 1500 / ext
  818). §1b's "extrinsics bundle ignores focal length" suspicion is real in the
  code but is not what separates these modules.

### The instrument that actually works: sweep the preview scale

`light validate --max-side 1024 → 128`. Residual misalignment shrinks with
resolution, so a merely *misaligned* module climbs toward its true correlation as
the preview coarsens, while a module in a *mirrored frame* gets more negative —
the anti-correlation is real signal and survives blurring.

| module | 1024 | 512 | 256 | 128 | reading |
| --- | --- | --- | --- | --- | --- |
| B2 | +0.286 | +0.400 | +0.505 | +0.584 | aligned, parallax-limited |
| B3 | +0.326 | +0.454 | +0.568 | +0.653 | aligned, parallax-limited |
| C5 | +0.243 | +0.376 | +0.596 | **+0.679** | aligned, parallax-limited |
| C6 | +0.146 | +0.228 | +0.342 | +0.412 | aligned, parallax-limited |
| C1 | +0.068 | +0.098 | +0.127 | +0.148 | weakly aligned |
| C3 | +0.012 | +0.014 | +0.025 | +0.027 | no correlation at any scale |
| C4 | −0.007 | −0.012 | −0.010 | −0.021 | no correlation at any scale |
| C2 | −0.082 | −0.131 | −0.176 | **−0.252** | **mirrored** — diverges, not converges |

This retires the "C row is unmeasurable" worry from §1. **C5 reaches +0.68 —
better than any B module.** A 150 mm module at ~20 % overlap scores perfectly
well when its pose is right. The low full-resolution numbers are parallax:
`homography_infinity` discards `t`, and ignored disparity scales with `fx`, so
the C row pays roughly 2.5–3× what the B row pays for the same baseline. That
caps everyone's absolute score and is Phase 3's problem, not a pose bug.

So the C row splits three ways, and only one part is a mirror problem:
**C2 is mirrored** (and takes the `flag_true` flip to +0.272 on `00003`, +0.115
on `00045`); **C3 and C4 correlate with nothing at any scale**, which is a pose
or content question, not a parity one; **C1 is fine**, merely weak.

### What broke the §1 flip rule

Running the flip experiment at 128 px, where the signal is 2–3× stronger:

| capture | ref | B1 no flip | B1 flip | B5 no flip | B5 flip |
| --- | --- | --- | --- | --- | --- |
| `00078` | **A1** | −0.415 | **+0.548** | −0.409 | **+0.516** |
| `00003` | **B4** | +0.163 | +0.154 | **+0.129** | −0.003 |
| `00045` | **B4** | +0.186 | +0.195 | **+0.231** | +0.001 |

The same module wants opposite treatment depending on **which camera the file
names as reference**. `flip_img_around_x` is byte-identical for every module
across these files, so this is not a firmware or per-unit difference.

That kills "flip iff `flag == false`". It was inferred from `00078` — the only
capture in the set with an A-row reference — and it is an artifact of the
reference, not a property of B1/B5. The old failure mode exactly: a rule tuned on
the subset that cannot contradict it.

### The question this leaves, which is sharper than the one it replaces

Why does the flip requirement depend on the reference camera at all? Two frames
are involved: A1 is `MirrorType::None`, B4 is `Glued`, and both take the
`canonical` path. If those two canonical poses were in the same convention,
B1/B5 could not need a flip against one and not the other.

So §1b's convention suspicion is alive, but pointed somewhere new: not
`canonical` vs `mirror`, but **`None` vs `Glued` inside the canonical path**.

### Two follow-ups, both answered without the camera

**The fixture cannot be shot.** All 62 captures scanned
(`cargo run --release -p light --example fixture_scan -- …`): the camera fires in
two exclusive modes — **wide** (A row + B row, 10 modules, reference A1, focal ≤
66) or **tele** (B row + C row, 11 modules, reference B4, focal ≥ 71). The A row
and the glued C modules never appear in the same shot, on any capture, and that
is a property of how the camera selects modules, not a gap in our sample. An
earlier note here said to go shoot one; that was wrong and is withdrawn. The two
reference classes cannot be compared by correlation at all. Only the B row
bridges the two modes, which is exactly why B1/B5 are where the contradiction
surfaced.

**The convention mismatch is not in `t`.** `mirror_aim` now reconstructs every
camera centre (`C = −Rᵀ·t`) and checks the physical layout — no images involved,
which is the point. The sixteen centres form three clean depth layers:

| row | mean z | spread | contains |
| --- | --- | --- | --- |
| A | +0.00 mm | 0.00 mm | 5 canonical (`None`) |
| B | −10.36 mm | 1.24 mm | 4 mirror + **B4 canonical (`Glued`)** |
| C | −12.43 mm | 1.52 mm | 4 mirror + **C5/C6 canonical (`Glued`)** |

The layering itself is physics, not a defect: a folded optical path puts the
virtual centre behind the face, so the mirror rows sit back from the flat-mounted
A row.

What matters is that **canonical and mirror-derived poses interleave within a
row**. B4 lands 0.19 mm from the plane its four mirror-derived neighbours define;
C5 and C6 land 0.04 and 0.14 mm from theirs. Two poses computed by different code
paths from different stored data agree on where the module physically is, to
fractions of a millimetre. That is not what a convention mismatch looks like.

So `t` is exonerated for both paths, and since `homography_infinity` uses only
`K` and `R` anyway, **the reference-dependent flip has to live in `R`**. That is
where to look next, and it can be looked at entirely offline.

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

## 6. Dataset hygiene — cleared 2026-07-21

61 captures in `.data-from-camera/raw/` (gitignored). The worry was a live GPS
fix: the camera app writes one into a trailing `LELR` block (`TYPE_GPS_DATA = 2`,
`FUSION.md`), so sample captures could hand a stranger the owner's address along
with the test data.

**There is none.** Three independent checks over all 62 files agree:

| check | result |
| --- | --- |
| `FusionMeta.gps` after decode (`privacy_scan` example) | 0 of 62 |
| raw scan for trailing `LELR` blocks with `type = 2` | 0 — only 247 × type 0, 2 × type 1 |
| `Exif\0\0` + GPS IFD pointer (`0x8825`) in the container | no EXIF at all in any file |

Checked three ways on purpose. A parser that quietly skips a block would report
a clean result for the worst possible reason, and this is the one question where
a false all-clear does the damage.

So nothing needs stripping before publishing samples from *this* set. Keep
`privacy_scan` and re-run it on any capture arriving from an owner — location
services were evidently off on this camera, which is a property of these files,
not of the format.

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
