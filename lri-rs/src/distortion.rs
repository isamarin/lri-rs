//! Polynomial lens distortion from `ltpb.Distortion.Polynomial`.

#[derive(Clone, Debug, PartialEq)]
pub struct PolynomialDistortion {
	pub center: [f32; 2],
	pub normalization: [f32; 2],
	pub coeffs: Vec<f32>,
}

impl PolynomialDistortion {
	/// Normalized coords from pixel `(u, v)`.
	pub fn pixel_to_normalized(&self, u: f32, v: f32) -> (f32, f32) {
		let nx = self.normalization[0].max(1e-6);
		let ny = self.normalization[1].max(1e-6);
		(
			(u - self.center[0]) / nx,
			(v - self.center[1]) / ny,
		)
	}

	/// Pixel coords from normalized (distorted) coords.
	pub fn normalized_to_pixel(&self, xn: f32, yn: f32) -> (f32, f32) {
		(
			xn * self.normalization[0] + self.center[0],
			yn * self.normalization[1] + self.center[1],
		)
	}

	/// Forward: undistorted normalized → distorted normalized.
	pub fn distort_normalized(&self, xn: f32, yn: f32) -> (f32, f32) {
		let r2 = xn * xn + yn * yn;
		let mut scale = 1.0f32;
		let mut rp = r2;
		for &k in &self.coeffs {
			scale += k * rp;
			rp *= r2;
		}
		(xn * scale, yn * scale)
	}

	/// Inverse: distorted normalized → undistorted normalized (fixed-point).
	pub fn undistort_normalized(&self, xd: f32, yd: f32) -> (f32, f32) {
		let mut xn = xd;
		let mut yn = yd;
		for _ in 0..12 {
			let (fx, fy) = self.distort_normalized(xn, yn);
			xn += xd - fx;
			yn += yd - fy;
		}
		(xn, yn)
	}

	/// Undistorted pixel from distorted pixel.
	pub fn undistort_pixel(&self, u: f32, v: f32) -> (f32, f32) {
		let (xd, yd) = self.pixel_to_normalized(u, v);
		let (xn, yn) = self.undistort_normalized(xd, yd);
		self.normalized_to_pixel(xn, yn)
	}
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModuleDistortion {
	pub polynomial: Option<PolynomialDistortion>,
	pub has_cra: bool,
}

impl ModuleDistortion {
	pub fn has_polynomial(&self) -> bool {
		self.polynomial.is_some()
	}

	pub fn poly_coeffs(&self) -> usize {
		self.polynomial.as_ref().map(|p| p.coeffs.len()).unwrap_or(0)
	}
}

/// Remap grayscale image: output pixel samples undistorted geometry.
pub fn undistort_gray(
	src: &[u8],
	width: u32,
	height: u32,
	model: &PolynomialDistortion,
) -> Vec<u8> {
	let w = width as i32;
	let h = height as i32;
	let mut out = vec![0u8; src.len()];
	for y in 0..height {
		for x in 0..width {
			let (su, sv) = model.undistort_pixel(x as f32, y as f32);
			let i = (y * width + x) as usize;
			out[i] = sample_bilinear_u8(src, w, h, su, sv);
		}
	}
	out
}

fn sample_bilinear_u8(src: &[u8], w: i32, h: i32, x: f32, y: f32) -> u8 {
	if x < 0.0 || y < 0.0 || x >= (w - 1) as f32 || y >= (h - 1) as f32 {
		return 0;
	}
	let x0 = x.floor() as i32;
	let y0 = y.floor() as i32;
	let fx = x - x0 as f32;
	let fy = y - y0 as f32;
	let idx = |px: i32, py: i32| (py * w + px) as usize;
	let p00 = src[idx(x0, y0)] as f32;
	let p10 = src[idx(x0 + 1, y0)] as f32;
	let p01 = src[idx(x0, y0 + 1)] as f32;
	let p11 = src[idx(x0 + 1, y0 + 1)] as f32;
	let top = p00 * (1.0 - fx) + p10 * fx;
	let bot = p01 * (1.0 - fx) + p11 * fx;
	(top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn roundtrip_distort_undistort_normalized() {
		let m = PolynomialDistortion {
			center: [100.0, 100.0],
			normalization: [2000.0, 2000.0],
			coeffs: vec![0.01, -0.002, 0.0, 0.0, 0.0],
		};
		let (xd, yd) = m.distort_normalized(0.1, -0.05);
		let (xn, yn) = m.undistort_normalized(xd, yd);
		assert!((xn - 0.1).abs() < 1e-3);
		assert!((yn + 0.05).abs() < 1e-3);
	}

	#[test]
	fn l16_a1_polynomial_extracted() {
		let Some(bytes) = crate::fixtures::l16_00078_bytes() else {
			return;
		};
		let lri = crate::LriFile::decode(&bytes).expect("decode");
		let a1 = lri
			.fusion
			.module_geometry
			.iter()
			.find(|m| m.camera == crate::CameraId::A1)
			.expect("A1");
		let poly = a1
			.distortion
			.polynomial
			.as_ref()
			.expect("A1 polynomial");
		assert!(poly.coeffs.len() >= 3, "expected >=3 coeffs, got {}", poly.coeffs.len());
		assert!(poly.normalization[0] > 100.0);
	}
}