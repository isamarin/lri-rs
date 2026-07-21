# Light patents — relevance notes

What the patents do and don't give us.

**TL;DR (revised):** the architecture patent (`9,563,033`) confirms the pipeline
but has no mechanics. The *camera-device* family (`9,857,584`, `10,353,195`,
`US20170123189A1`) is a separate line of filings and **is directly relevant to
the `mirror_pose` blocker** — not because it gives formulas (it does not), but
because it establishes that mirrors come in **three classes, keyed by focal
group**. That is a structural constraint the current code does not model.

> Superseded: an earlier revision of this file said the patents were "useless
> for `mirror_pose`" and that the IP went to John Deere. Both were wrong — see
> below.

## Assignment chain — corrected

The portfolio **split**. Conflating the two branches produced the earlier error.

| Branch | Where it went |
| ------ | ------------- |
| Clarity / automotive vision | LGT (ABC), LLC → Blue River / **John Deere** (2022) |
| **Camera + optical-chain + mirror patents** | Light Labs Inc. → **Samsung Electronics Co., Ltd.** (recorded 2021-02-09) |

Full chain on the camera branch: The Lightco Inc. → Light Labs Inc. (name
change, 2018-03-29) → Samsung Electronics (2021-02-09).

Consequence: the mirror geometry we care about sits in a portfolio held by a
**passive acquirer of a dead product line**, not by an operator building on it.

## Patent 1 — architecture

**US 9,563,033 B2** — *Methods and apparatus for capturing images and/or for
using captured images*. Inventor: Rajiv Laroia.

Multiple optical chains of differing focal length; wide modules capture the
whole scene, tele modules capture different (partially overlapping) parts; raw
images stored or output; composite assembled using **depth from parallax**;
composite has a higher pixel count than any single frame.

| Claim / idea | Our pipeline |
| ------------ | ------------ |
| Wide + several tele on different zones | The A/B/C row architecture (see FUSION.md) |
| Depth from stereo / parallax | Confirms depth is the load-bearing stage, and that **SGM is the right path**, not a ToF dependency (OPEN-QUESTIONS #2) |
| "Store or output" raw images | Justifies `.lri` as a raw container |
| Non-overlapping portions + higher pixel count | Why tele modules exist, how 50+ MP is reached |
| Post-capture combining | Lumen fuses on the desktop, not only on-camera |

Does **not** disclose: mirror pose math, WarpField formulas, protobuf layout,
SGM / cost-volume specifics.

## Patent family 2 — camera device and components ← relevant to blocker #1

**US 9,857,584 B2** and **US 10,353,195 B2** — *Camera device methods, apparatus
and components*; **US20170123189A1** / **US20160205326A1** — *Methods and
apparatus for implementing and/or using a camera device*.

### 2a. Three mirror classes, not one — the load-bearing finding

> "The camera modules used to capture the full scene area of interest have
> **fixed mirrors** while the camera modules used to capture small portions of
> the scene area of interest each include a **movably hinged mirror**."
> — US 10,353,195

And a third class with no mirror at all:

> "optical chains […] having the smallest diameter outer openings […] and
> smallest focal lengths are implemented using optical chains which **do not use
> mirrors** and extend straight toward the back" — US 9,857,584

> "optical chains having relatively short focal lengths may be implemented
> without the use of a light redirection element" — US20170123189A1

Module counts in the illustrative embodiment (**confidence: medium** — patent
figure numbering, not the shipping L16 naming; the *grouping* is what transfers,
not the counts):

| Focal group | Modules in embodiment | Mirror |
| ----------- | --------------------- | ------ |
| f₃ (longest) | 7 | movable hinged |
| f₂ (medium) | 5 | movable hinged |
| f₁ (shortest) | 5 | **none** — straight through |

Rationale given: long focal lengths do not fit the camera's depth, so the path
is folded sideways. Short ones fit and are not folded.

### 2b. The hinge axis is constrained, not free

> "The axis of the hinge is perpendicular to the Part B of the optical axis and
> parallel to the plane of the front face of the camera 600."

> "the hinge 508 prevents motion in other directions and thus the optical axis
> (outside the camera) **rotates in a plane perpendicular to the axis of the
> hinge**."

Nominal geometry:

> "When the mirror 510 is at a **45 degree angle**, the light entering the
> opening 512 along it's optical axis is deflected **90 degrees** into the
> optical axis of Part B."

So: one rotational degree of freedom, axis fixed by construction, nominal 45°
mirror → 90° deflection, deviation from 45° steers the module.

### 2c. What the family still does NOT give

- **No image-flip / inversion / handedness discussion anywhere.** Checked across
  all four documents. `flip_img_around_x` has no patent basis — it stays an
  empirical or RE question.
- No mirror-angle → view-shift formula (the 2× reflection relation is never
  written out).
- No calibration or geometric registration methodology.
- No coordinate-system definition or sign conventions.

## Why this matters for `mirror_pose` (OPEN-QUESTIONS #1)

Observed NCC against reference B4 (capture `L16_00003`, focal 77):

```
B4←B2  0.58   ok
B4←B3  0.49   ok
B4←B1  0.11   near zero
B4←B5  0.01   near zero
B4←C1..C4     negative
```

`flip_img_around_x` was tuned blind on B2/B3 only (commit `cff7a7d`), i.e. on
**movable-mirror modules of a single focal group**. The patents say that is not
the only class present. Near-zero (B1/B5) reads as misregistration inside a
class; systematically negative (C row) reads as the *wrong model applied*, not a
wrong sign.

**Hypothesis (confidence: medium-high — patent-backed, not yet verified against
our data):** mirror handling is a **table keyed by focal group / mirror class**
— movable, fixed, none — while the code applies one global rule.

Cheap test, no RE required: run the sign/flip search **separately per row** and
see whether B and C converge on *different* solutions. If they do, the constant
is really a table, and blocker #1 is a modelling bug rather than a sign bug.

Second constraint to apply immediately: the hinge axis is **not a free
parameter**. If `mirror_pose.rs` searches the rotation axis freely, restrict it
to "perpendicular to the post-mirror optical axis, parallel to the camera face"
and one of the three unknowns collapses.

## Practical takeaway

1. Patents give **structure, never mechanics** — but structure is exactly what
   blocker #1 turned out to need.
2. `flip_img_around_x` must still come from `libcp.so` (`0x1c7580`) or from
   data. Nothing public documents it.
3. SGM remains the right call for OPEN-QUESTIONS #2, backed by `9,563,033`.

## Sources

- [US 9,563,033 B2](https://patents.google.com/patent/US9563033B2/en)
- [US 9,857,584 B2](https://patents.google.com/patent/US9857584B2/en)
- [US 10,353,195 B2](https://patents.google.com/patent/US10353195B2/en)
- [US20170123189A1](https://patents.google.com/patent/US20170123189A1/en)
- [US20160205326A1](https://patents.google.com/patent/US20160205326A1/en)
- [US20150138423A1](https://patents.google.com/patent/US20150138423A1/en)
- [Patents assigned to Light Labs Inc. — Justia](https://patents.justia.com/assignee/light-labs-inc)
- [Rajiv Laroia — Justia](https://patents.justia.com/inventor/rajiv-laroia)
