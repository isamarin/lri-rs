# Light patents — relevance notes

What the patents do and don't give us. TL;DR: they describe the *architecture*
(wide + several tele, depth from parallax, post-capture combine) — which
independently confirms our reverse-engineered pipeline — but contain none of the
engine mechanics (mirror pose, WarpField, SGM, protobuf). For the current
`mirror_pose` blocker they're useless.

## Primary patent

**US 9,563,033 B2** — *Methods and apparatus for capturing images and/or for
using captured images*
Inventor: **Rajiv Laroia**. Assignee: Light Labs Inc. → LGT (ABC), LLC / Blue
River (John Deere).

### Gist (from the claims)

A camera with multiple **optical chains** (modules) of differing focal length:

- One or more **wide** modules (short FL) capture the whole scene.
- Several **tele** modules (longer FL) capture *different parts* of that scene
  (partially overlapping / non-overlapping).
- Raw images are stored or output → fusion can happen after capture.
- A composite is assembled using **depth information** from parallax (stereo).
- The composite has a higher pixel count than any single frame.

### Mapping to our pipeline

| Claim / idea | Our pipeline |
| ------------ | ------------ |
| Wide + several tele on different zones | The A/B/C row architecture (see FUSION.md) |
| Depth from stereo / parallax | Confirms depth is the load-bearing stage — and that **SGM is the right path**, not a ToF dependency (see OPEN-QUESTIONS #2) |
| "Store or output" raw images | Justifies `.lri` as a raw container |
| Non-overlapping portions + higher pixel count | Why tele modules exist and how 50+ MP is reached |
| Post-capture combining | Lumen fuses on the desktop, not only on-camera |

### What it does NOT disclose

- The `mirror_pose` / movable-mirror math (the current blocker)
- WarpField formulas
- Protobuf layout
- Concrete SGM / cost-volume algorithms

Only the high-level architecture of what Light intended.

## Other patents

Adjacent ones exist (user settings, post-capture control), but the strongest and
most directly relevant is **US 9,563,033 B2**.

## Practical takeaway

One usable signal: the patent's "depth from parallax/stereo" language backs the
decision to build **per-pixel SGM depth** (OPEN-QUESTIONS #2) rather than lean on
`tof_range`. Everything mechanical still has to come from `libcp.so` + real data.
