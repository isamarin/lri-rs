//! Homographies for planar / depth-guided warps between calibrated modules.

use nalgebra::{Matrix3, Vector3};

#[derive(Clone, Copy, Debug)]
pub struct CameraPose {
	pub k: Matrix3<f64>,
	pub r: Matrix3<f64>,
	pub t: Vector3<f64>,
}

impl CameraPose {
	pub fn from_row_major(k: [f32; 9], r: [f32; 9], t: [f32; 3]) -> Self {
		Self {
			k: mat3_f32(k),
			r: mat3_f32(r),
			t: Vector3::new(t[0] as f64, t[1] as f64, t[2] as f64),
		}
	}

	/// Optical centre in world coordinates: `C = −Rᵀ·t`.
	///
	/// Where the module physically sits. Unlike anything derived from image
	/// content, this is checkable against the hardware: the modules of a rig are
	/// mounted in a known arrangement, so centres that do not form it indicate a
	/// pose stored in a convention the reader is not applying.
	pub fn centre(&self) -> Vector3<f64> {
		-(self.r.transpose() * self.t)
	}

	/// Viewing direction in world coordinates: `Rᵀ·ẑ`, the third row of `R`.
	pub fn optical_axis(&self) -> Vector3<f64> {
		Vector3::new(self.r[(2, 0)], self.r[(2, 1)], self.r[(2, 2)])
	}

	/// Determinant of `R`. A pose is a rotation only at `+1`; `−1` means a
	/// reflection is baked in and the module sits in a mirrored frame.
	/// Orthogonality (`RᵀR = I`) holds for both and cannot tell them apart.
	pub fn rotation_determinant(&self) -> f64 {
		self.r.determinant()
	}

	pub fn scaled(&self, step: usize) -> Self {
		let s = 1.0 / step as f64;
		let mut k = self.k;
		k[(0, 0)] *= s;
		k[(0, 2)] *= s;
		k[(1, 1)] *= s;
		k[(1, 2)] *= s;
		Self { k, r: self.r, t: self.t }
	}
}

pub fn mat3_f32(row_major: [f32; 9]) -> Matrix3<f64> {
	Matrix3::new(
		row_major[0] as f64,
		row_major[1] as f64,
		row_major[2] as f64,
		row_major[3] as f64,
		row_major[4] as f64,
		row_major[5] as f64,
		row_major[6] as f64,
		row_major[7] as f64,
		row_major[8] as f64,
	)
}

/// Planar homography at infinity: `x_ref ~ K_ref (R_ref R_src^T) K_src^-1 x_src`.
pub fn homography_infinity(src: &CameraPose, dst: &CameraPose) -> Matrix3<f64> {
	let r_rel = dst.r * src.r.transpose();
	let k_src_inv = src.k.try_inverse().expect("singular K_src");
	dst.k * r_rel * k_src_inv
}

/// Homography induced by plane `Z = depth` mm in the **destination** camera frame.
pub fn homography_at_depth(src: &CameraPose, dst: &CameraPose, depth_mm: f64) -> Matrix3<f64> {
	let depth = depth_mm.max(1.0);
	let r_rel = dst.r * src.r.transpose();
	let t_rel = dst.t - r_rel * src.t;
	let n = Vector3::new(0.0, 0.0, 1.0);
	let k_src_inv = src.k.try_inverse().expect("singular K_src");
	// Plane Z_src = depth in the source frame: x_dst ~ K_dst (R_rel + t_rel n^T / depth) K_src^-1 x_src.
	// The parallax term is +, not - (the textbook minus assumes n^T X + d = 0 with d < 0).
	dst.k * (r_rel + t_rel * n.transpose() / depth) * k_src_inv
}

/// Map reference pixel through `H` into source frame: `x_src ~ H^-1 x_ref`.
pub fn map_ref_to_src(h: &Matrix3<f64>, x: f64, y: f64) -> Option<(f64, f64)> {
	let h_inv = h.try_inverse()?;
	let p = h_inv * Vector3::new(x, y, 1.0);
	if p.z.abs() < 1e-12 {
		return None;
	}
	Some((p.x / p.z, p.y / p.z))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn scaled_brings_intrinsics_down_with_the_preview() {
		let pose = CameraPose::from_row_major(
			[200.0, 0.0, 100.0, 0.0, 200.0, 80.0, 0.0, 0.0, 1.0],
			[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
			[0.0; 3],
		)
		.scaled(4);
		assert!((pose.k[(0, 0)] - 50.0).abs() < 1e-9);
		assert!((pose.k[(0, 2)] - 25.0).abs() < 1e-9);
		assert!((pose.k[(1, 1)] - 50.0).abs() < 1e-9);
		assert!((pose.k[(1, 2)] - 20.0).abs() < 1e-9);
	}

	#[test]
	fn centre_and_determinant_of_a_plain_pose() {
		let pose = CameraPose::from_row_major(
			[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
			[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
			[3.0, -4.0, 5.0],
		);
		// R = I ⇒ C = −t.
		assert_eq!(pose.centre(), Vector3::new(-3.0, 4.0, -5.0));
		assert!((pose.rotation_determinant() - 1.0).abs() < 1e-12);
	}

	fn identity_pose(f: f64) -> CameraPose {
		CameraPose {
			k: Matrix3::new(f, 0.0, 50.0, 0.0, f, 40.0, 0.0, 0.0, 1.0),
			r: Matrix3::identity(),
			t: Vector3::zeros(),
		}
	}

	#[test]
	fn infinity_homography_is_identity_for_same_pose() {
		let p = identity_pose(100.0);
		let h = homography_infinity(&p, &p);
		let mapped = map_ref_to_src(&h, 120.0, 80.0).unwrap();
		assert!((mapped.0 - 120.0).abs() < 1e-4);
		assert!((mapped.1 - 80.0).abs() < 1e-4);
	}

	#[test]
	fn finite_plane_parallax_has_correct_sign() {
		// src at origin, dst translated +10 along X (world->cam t). A point on the
		// principal ray at depth 1000 projects to (cx,cy)=(50,40) in src and must
		// land at (51,40) in dst: +1 px parallax, not -1.
		let src = identity_pose(100.0);
		let mut dst = identity_pose(100.0);
		dst.t = Vector3::new(10.0, 0.0, 0.0);
		let h = homography_at_depth(&src, &dst, 1000.0);
		let p = h * Vector3::new(50.0, 40.0, 1.0);
		let (x, y) = (p.x / p.z, p.y / p.z);
		assert!((x - 51.0).abs() < 1e-6, "x={x} expected 51 (got wrong parallax sign?)");
		assert!((y - 40.0).abs() < 1e-6, "y={y} expected 40");
	}

	#[test]
	fn deep_plane_approaches_infinity_homography() {
		let src = identity_pose(100.0);
		let mut dst = identity_pose(100.0);
		dst.t = Vector3::new(10.0, 5.0, 0.5);
		let h_inf = homography_infinity(&src, &dst);
		let h_far = homography_at_depth(&src, &dst, 1.0e6);
		let p = Vector3::new(130.0, 90.0, 1.0);
		let a = h_inf * p;
		let b = h_far * p;
		assert!((a.x / a.z - b.x / b.z).abs() < 0.01);
	}
}