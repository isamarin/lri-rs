//! Sampling, warping and overlap scoring on 8-bit grayscale buffers.
//!
//! Deliberately takes `&[u8]` plus dimensions rather than an image type. This
//! crate is the geometry core and depends only on `nalgebra`; letting an image
//! library in through here would make every consumer inherit that choice. The
//! callers already hold something with `.as_raw()`.
//!
//! **Zero means "no data" throughout.** Warping leaves it outside the source
//! footprint, and scoring skips any pixel where either side is zero — otherwise
//! the un-warped border dominates every correlation.

use nalgebra::{Matrix3, Vector3};

/// Bilinear sample. Returns 0 outside the buffer, which is the no-data value.
pub fn sample_bilinear(data: &[u8], width: u32, height: u32, x: f64, y: f64) -> u8 {
	let (w, h) = (width as i64, height as i64);
	if x < 0.0 || y < 0.0 || x >= (w - 1) as f64 || y >= (h - 1) as f64 {
		return 0;
	}
	let x0 = x.floor() as i64;
	let y0 = y.floor() as i64;
	let fx = x - x0 as f64;
	let fy = y - y0 as f64;
	let at = |px: i64, py: i64| data[(py * w + px) as usize] as f64;
	let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1, y0) * fx;
	let bot = at(x0, y0 + 1) * (1.0 - fx) + at(x0 + 1, y0 + 1) * fx;
	(top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8
}

/// Warp `src` into the destination frame by inverse-mapping through `h`.
///
/// `h` maps **source → destination**, so each destination pixel is looked up
/// through `h⁻¹`. A singular `h` yields an empty (all-zero) result rather than a
/// panic: a degenerate pose is data, not a bug in the caller.
pub fn warp_inverse(
	src: &[u8],
	src_size: (u32, u32),
	h: &Matrix3<f64>,
	out_size: (u32, u32),
) -> Vec<u8> {
	let (ow, oh) = out_size;
	let mut out = vec![0u8; (ow as usize) * (oh as usize)];
	let Some(h_inv) = h.try_inverse() else {
		return out;
	};
	for y in 0..oh {
		for x in 0..ow {
			let p = h_inv * Vector3::new(x as f64, y as f64, 1.0);
			if p.z.abs() < 1e-9 {
				continue;
			}
			out[(y as usize) * (ow as usize) + x as usize] =
				sample_bilinear(src, src_size.0, src_size.1, p.x / p.z, p.y / p.z);
		}
	}
	out
}

/// Agreement between two buffers over the pixels where both carry data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlap {
	/// Pixels where neither side was zero. Read the scores against this — a few
	/// hundred overlapping pixels can correlate strongly on noise alone.
	pub pixels: u32,
	/// Mean absolute difference. `NaN` when nothing overlapped.
	pub mae: f64,
	/// Zero-mean normalized cross-correlation. `NaN` when nothing overlapped or
	/// when either side is flat, since correlation is undefined without variance.
	pub ncc: f64,
}

/// Score two equally-sized buffers over their common non-zero footprint.
pub fn compare_overlap(a: &[u8], b: &[u8]) -> Overlap {
	assert_eq!(a.len(), b.len(), "compare_overlap needs equal buffers");
	let (mut n, mut mae) = (0u32, 0f64);
	let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0f64, 0f64, 0f64, 0f64, 0f64);

	for (pa, pb) in a.iter().zip(b.iter()) {
		if *pa == 0 || *pb == 0 {
			continue;
		}
		let (av, bv) = (*pa as f64, *pb as f64);
		n += 1;
		mae += (av - bv).abs();
		sa += av;
		sb += bv;
		saa += av * av;
		sbb += bv * bv;
		sab += av * bv;
	}

	if n == 0 {
		return Overlap { pixels: 0, mae: f64::NAN, ncc: f64::NAN };
	}
	let nf = n as f64;
	let cov = sab - sa * sb / nf;
	let var_a = saa - sa * sa / nf;
	let var_b = sbb - sb * sb / nf;
	let ncc = if var_a < 1e-6 || var_b < 1e-6 {
		f64::NAN
	} else {
		cov / (var_a.sqrt() * var_b.sqrt())
	};
	Overlap { pixels: n, mae: mae / nf, ncc }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn bilinear_interpolates_between_neighbours() {
		let data = [0u8, 100, 0, 100];
		assert_eq!(sample_bilinear(&data, 2, 2, 0.5, 0.0), 50);
	}

	#[test]
	fn bilinear_returns_no_data_outside() {
		let data = [10u8, 20, 30, 40];
		assert_eq!(sample_bilinear(&data, 2, 2, -0.1, 0.5), 0);
		assert_eq!(sample_bilinear(&data, 2, 2, 5.0, 0.5), 0);
	}

	#[test]
	fn identity_warp_preserves_the_image() {
		let src = vec![7u8; 16];
		let out = warp_inverse(&src, (4, 4), &Matrix3::identity(), (4, 4));
		// Edges sample outside and come back as no-data; the interior survives.
		assert_eq!(out[5], 7);
	}

	#[test]
	fn singular_homography_yields_no_data_not_a_panic() {
		let src = vec![9u8; 16];
		let out = warp_inverse(&src, (4, 4), &Matrix3::zeros(), (4, 4));
		assert!(out.iter().all(|p| *p == 0));
	}

	#[test]
	fn identical_buffers_correlate_perfectly() {
		let a = [10u8, 40, 90, 160];
		let o = compare_overlap(&a, &a);
		assert_eq!(o.pixels, 4);
		assert!(o.mae.abs() < 1e-9);
		assert!((o.ncc - 1.0).abs() < 1e-9);
	}

	#[test]
	fn inverted_buffers_correlate_negatively() {
		let a = [10u8, 40, 90, 160];
		let b = [160u8, 90, 40, 10];
		assert!(compare_overlap(&a, &b).ncc < -0.9);
	}

	#[test]
	fn zeros_are_excluded_from_the_overlap() {
		let a = [0u8, 40, 90, 0];
		let b = [77u8, 40, 90, 12];
		assert_eq!(compare_overlap(&a, &b).pixels, 2);
	}

	#[test]
	fn no_overlap_is_nan_not_zero() {
		// Zero would read as "measured, uncorrelated". Nothing was measured.
		let o = compare_overlap(&[0u8, 0], &[5u8, 9]);
		assert_eq!(o.pixels, 0);
		assert!(o.ncc.is_nan());
	}

	#[test]
	fn flat_input_has_no_defined_correlation() {
		let flat = [50u8; 4];
		let vary = [10u8, 40, 90, 160];
		assert!(compare_overlap(&flat, &vary).ncc.is_nan());
	}
}
