# openfusion

Open reconstruction of the **Light L16** 16→1 fusion — the geometry core.

Standalone Rust crate (depends only on `nalgebra`): planar/depth homographies
and a plane-sweep depth MVP used to align the L16's exposed camera modules into
one frame. The decoder side — `.lri` parsing, calibration, mirror pose — lives
in [`lri-rs`](https://github.com/isamarin/lri-rs); this crate is the fusion math
layered on top.

Part of the openfusion project: an open-source revival decoding and re-fusing
the Light L16, worked against a real camera on the bench.

## Modules

| Module | Role |
| ------ | ---- |
| `warp` | `CameraPose`, planar homography at infinity, plane-induced homography at depth |
| `stereo` | plane-sweep depth search, zero-mean normalized cross-correlation |

## License

ISC / MIT.
