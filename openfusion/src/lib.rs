//! openfusion — geometry core for reconstructing the Light L16's 16→1 combine.
//!
//! Standalone (depends only on `nalgebra`): planar/depth homographies and a
//! plane-sweep depth MVP. The decoder side (`.lri` parsing, calibration,
//! mirror pose) lives in `lri-rs`; this crate is the fusion math on top.

pub mod stereo;
pub mod warp;

pub use warp::CameraPose;
