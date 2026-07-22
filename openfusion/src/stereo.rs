//! Coarse depth estimation (plane sweep MVP before full SGM).

use crate::warp::{homography_at_depth, CameraPose};

/// Scan frontal plane depths; `score(depth_mm)` should return higher for better alignment.
pub fn plane_sweep<F>(depth_min_mm: f64, depth_max_mm: f64, steps: usize, mut score: F) -> (f64, f64)
where
	F: FnMut(f64) -> f64,
{
	assert!(steps >= 2);
	let mut best_z = depth_min_mm;
	let mut best = f64::NEG_INFINITY;
	for i in 0..steps {
		let t = i as f64 / (steps - 1) as f64;
		let z = depth_min_mm + t * (depth_max_mm - depth_min_mm);
		let s = score(z);
		if s > best {
			best = s;
			best_z = z;
		}
	}
	(best_z, best)
}

/// Homography for warping `src` into `dst` at plane depth `depth_mm`.
pub fn warp_homography(src: &CameraPose, dst: &CameraPose, depth_mm: f64) -> nalgebra::Matrix3<f64> {
	homography_at_depth(src, dst, depth_mm)
}

/// Zero-mean normalized cross-correlation on overlapping non-zero pixels.
///
/// Thin wrapper kept for callers that want the single number. Use
/// [`crate::raster::compare_overlap`] when the overlap size matters — and it
/// usually does, since a sliver of overlap correlates strongly on noise alone.
pub fn ncc_overlap(a: &[u8], b: &[u8]) -> f64 {
	crate::raster::compare_overlap(a, b).ncc
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn plane_sweep_picks_peak() {
		let (z, s) = plane_sweep(0.0, 100.0, 101, |d| -(d - 42.0).powi(2) as f64);
		assert!((z - 42.0).abs() < 1.0);
		assert!(s >= -1.0);
	}

	#[test]
	fn ncc_identical_vectors() {
		let v = vec![10u8, 20, 30, 40];
		let n = ncc_overlap(&v, &v);
		assert!((n - 1.0).abs() < 1e-9);
	}
}