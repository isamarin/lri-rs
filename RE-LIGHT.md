# re::light — plan

A camera that shipped in 2017, was abandoned in 2019, and whose maker no longer
exists. Sixteen sensors, a custom ASIC, a fusion pipeline nobody outside the
company ever saw. The hardware still powers on. The software that made it worth
owning is gone.

The goal is not a parser. The goal is that an L16 on a shelf becomes a camera
again, without LightOS and without permission.

**And then that it stays alive without us.** The end state is handover: tools,
knowledge and archive in the community's hands, working whether or not any
particular person keeps caring. A revival that depends on one maintainer has
only moved the single point of failure, not removed it.

That reframes what the deliverable is. The code can be rewritten by anyone
competent. What cannot be cheaply re-derived is **what we learned about the
engine** — which is why `FUSION.md` carries confidence tags, `OPEN-QUESTIONS.md`
records superseded hypotheses, and `PATENTS.md` cites sources. Those documents
are the product; the crates are an implementation of them.

Written 2026-07-21. Revise as gates are passed, not on a schedule.

---

## Where we actually are

Two halves of the project are at very different maturity. Treating them as one
project is the main planning error to avoid.

| | state | score |
| --- | --- | --- |
| **Extraction** — `.lri` parse, 16-module RAW → DNG, GUI | works, tested on 61 captures | **8/10** |
| **Fusion** — geometry, warp, blend | root cause of the blocker found 2026-07-21, **not fixed** | **4/10** |

Assets on hand: a working L16 on USB (fw 1.3.5.1), 61 real captures, both native
engines extracted (`libcp.so` ARM + `libcp.dylib`), a Lumen reference render for
`L16_00078`, the patent family mapped ([`PATENTS.md`](PATENTS.md)), and a
validation harness that scores our output against Lumen's.

The field is empty. Upstream `lri-rs` has been dormant since 2024-03; the
community archive since 2024-09. There is no race — only an abandoned site.

---

## Principles

These are conclusions from mistakes already made, not general advice.

**Ship extraction and fusion separately.** They serve different people and have
different readiness. Coupling them delays the useful half by months.

**Cameras before attention.** What blocks fusion is not effort, it is a sample
size of one. Broad announcements bring traffic; owners bring firmware variants
and captures. Go where the hardware is first.

**Never announce what does not work.** The audience is ~150 people. Credibility
there is spent once.

**Never validate on a subset.** The parity bug survived because it was tuned on
B2/B3 — the only two modules where the flag happens to make it invisible. Every
geometry change is validated across *all* modules or it is not validated.

**An invariant beats a threshold.** `det(R) = +1` found in one run what NCC
tuning had not found in weeks. Prefer checks that can only be right or wrong.

---

## Phase 0 — hygiene · blocks everything

Not glamorous, genuinely blocking.

### 0.1 Licensing — done 2026-07-21

> Correction: an earlier revision of this file called this a "hard blocker" and
> claimed upstream had no licence. That was wrong — it came from trusting the
> GitHub API (`license: null`, because there is no `LICENSE` file) instead of
> reading the upstream README, which states ISC. Verify at the source.

Settled state:

| component | licence |
| --- | --- |
| **fork changes** | **AGPL-3.0-or-later**, isamarin × BLMK |
| upstream crates (`gennyble/lri-rs`) | ISC, gennyble — GPL-compatible |
| `lri-proto` | MIT, Daniel Lawrence Lu — GPL-compatible |
| combined work | **AGPL-3.0-or-later** |

Rationale in [`COPYRIGHT`](COPYRIGHT). Short version: use, fork and commerce are
all permitted; **enclosure is not**, and §13 extends that to network services so
"we host it, you rent it, source stays ours" is closed too. The community kept
this camera alive for years after its maker walked away; what they rebuilt
should not be shut back up by anyone, us included.

- [x] `LICENSE` (canonical AGPL-3.0), `COPYRIGHT` (per-component notices),
      `license` field in the workspace manifest, README section
- [ ] **Fix the `openlight-camera` licence claim.** MIT cannot be applied to a
      decompiled and recompiled proprietary APK; those rights are not ours to
      grant. Relabel honestly, or reconsider publishing it.
- [ ] Label the archive for what it is: preserved copies, rights with the
      original holders, kept because the originals are disappearing

Consequence to carry into Phase 1: **AGPL cannot flow back into ISC upstream.**
The contribute-back offer to `gennyble` has to be limited to changes we are also
willing to release under ISC. Say so plainly when making the offer.

### 0.2 Data and attribution

- [ ] Strip GPS blocks from the 61 captures; screen for recognizable locations
      (OPEN-QUESTIONS #6)
- [ ] Attribution prominent in every README: `luminat` forks
      `gennyble/lri-rs`, `light-l16` forks `helloavo/Light-L16-Archive`, with an
      explicit list of what is ours
- [ ] Decide, deliberately, what gets published: the format parser is clean
      reverse engineering and low risk; redistributed **firmware and the
      recompiled APK** are a different exposure — the camera IP now sits with
      Samsung (`PATENTS.md`). Archiving dead hardware is common practice; know
      the difference before publishing, don't discover it after.
- [ ] Repo topics (`light-l16`, `computational-photography`, `rust`, `dng`,
      `raw`) — currently unset, free discoverability
- [ ] Enable GitHub Discussions on `luminat`

**Gate:** nothing public until this is done.

---

## Phase 1 — ship extraction · "get your photos out"

The one thing 150 people with a dead camera actually want.

- [ ] Tag a `luminat` release: CLI + Tauri GUI, per-module DNG export
- [ ] README leads with the outcome, not the architecture: sixteen RAWs out of a
      camera whose maker is gone, opening in Lightroom
- [ ] Announce, in this order:
  1. **Issue on `helloavo/Light-L16-Archive`** — its README states the aim as
     "access individual Sensors, the RAW images contained in the .LRI Files…
     break free of LightOS". That is done. Highest-leverage single action
     available: it lands on the stated goal of a 150-star repo.
  2. **Issue or PR to `gennyble/lri-rs`** — contribute back rather than quietly
     diverge. Six issues are open there; some may close with our code. Settles
     attribution publicly and notifies 29 people who already care.
  3. **Discord** (`discord.gg/9ZzDYYQPp2`) and the **XDA firmware thread** — this
     is where the hardware is.
- [ ] Do **not** post to Hacker News or Reddit yet.
- [ ] Open a **compatibility registry** in the repo — a table owners extend
      themselves: firmware version, serial range, which modules behave oddly,
      what worked. Same shape as the classic Linux hardware compatibility lists.
      It is community infrastructure and it produces exactly the sample size the
      geometry work needs, in one move.
      **No personal data** — firmware and hardware behaviour only. No names, no
      emails, no owner table. This audience roots firmware and archives
      abandoned platforms; a registry of *people* would repel precisely those we
      need, and it serves no purpose here.

**Gate — the only metric that matters here:** a capture from *someone else's
camera*, ideally on a different firmware, extracted successfully. Target ≥ 2
independent cameras. Until then the geometry work rests on a sample of one.

---

## Phase 2 — parity · unblock fusion

Root cause is known (OPEN-QUESTIONS §1): `reflection_matrix` is Householder,
`det = −1` by construction, so the composed pose is improper unless
`flip_img_around_x` happens to be true. B1, B5, C1, C3 are left mirrored on every
capture and every focal length.

- [ ] Separate the flag's two jobs: image flip (what it means) vs parity of `R`
      (what it accidentally supplies)
- [ ] Make `R` proper by construction for every module; apply the image flip at
      warp time
- [ ] If empirics stall, disassemble `libcp.so` at `0x1c7580` with a precise
      question: *what restores parity when the flag is false?*
- [ ] Then attack C2/C4 — proper rotations, still negative NCC. Either angle/axis
      (§1a) or simply wrong pointing: C is 150 mm tele and, per the patents,
      those modules aim at different parts of the scene.

**Gate:** `det = +1` on all 16 modules across ≥ 5 captures (the red test in
`mirror_pose.rs` goes green), **and** NCC vs the reference positive for every
module that fired.

---

## Phase 3 — real depth

Residual softness is the single fronto-parallel plane.

- [ ] Replace `plane_sweep` with per-pixel depth → warp field; the engine's own
      `DepthToDisparity` says depth is mm along the optical axis, inverse-range
- [ ] Pick wide-baseline pairs (`libcp` warns `Baseline too small` for B4↔B1)
- [ ] Settle `tof:0.00` (OPEN-QUESTIONS #3): parsed wrong, absent, or genuinely
      unused as a seed

**Gate:** `blend_ncc_vs_lumen` in the validate harness. We already score our
output against Lumen's own render of `L16_00078` — that number, not opinion, is
the acceptance criterion. Set a threshold before starting, not after seeing it.

---

## Phase 4 — publish fusion

Only now is there something to announce.

- [ ] Split `openfusion` into its own repo + submodule (OPEN-QUESTIONS #5)
- [ ] Side-by-side: Lumen's render vs ours, with the NCC number stated
- [ ] Hacker News. "Reviving a dead computational camera" is a story; a parser is
      not. This is also the post ex-Light engineers are most likely to read.

**Gate:** we can state honestly that the open pipeline matches the proprietary
one within a stated margin.

---

## Phase 5 — handover

The point of the whole thing. Not an epilogue: if this does not happen, the
camera is alive only as long as one person stays interested.

- [ ] At least one more maintainer with commit rights, who has landed real
      changes — not a title handed out
- [ ] Contribution path that works without us: issue templates, a documented
      build, `OPEN-QUESTIONS.md` kept current so a newcomer can pick a task
- [ ] Everything under a license that permits forking without asking anyone
- [ ] Archive mirrored in more than one place — a preservation project that
      exists on a single account has not preserved anything
- [ ] Write down what is *not* understood as carefully as what is. The next
      person's starting point is our list of open questions, not our code.

**Gate:** we could stop tomorrow and the project would continue.

---

## Outreach — between Phase 1 and Phase 4

Write to ex-Light **imaging/calibration engineers** — not the founders. Names
come from the inventor lists on the patents; prefer people who left before the
IP transfers.

Timing: after the Phase 1 release (there is a shipped tool to point at), before
Phase 4 (no noise yet, and a specific open question still to ask).

Frame it as what it is — *the camera is dead and I am bringing it back* — and ask
about method in general terms, never "how did Light do X". Attribution precise,
forks named. This audience checks.

---

## Risks, honestly

- **Sample size of one.** Every geometry conclusion rests on a single camera and
  61 captures. Phase 1 exists largely to fix this.
- **Redistribution.** Firmware and the recompiled APK carry exposure the parser
  does not. Camera IP is Samsung's since 2021.
- **Solo project — the thing the plan exists to end.** Not a risk to mitigate but
  the condition to remove: handover is the goal, so every phase should leave
  behind something a stranger can pick up. Concretely, a second person with
  commit rights before Phase 4, and documentation good enough that they do not
  need to ask us anything.
- **Fusion may not fully close.** C2/C4 are unexplained; the ASIC may do things
  the desktop engine does not expose. Phase 1 must be able to stand alone if
  Phase 3 never reaches parity with Lumen.

## Not doing

- Rewriting LightOS or shipping a camera firmware
- Supporting hardware nobody has
- Chasing stars before the tool is honest
