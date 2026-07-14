use std::fs::File;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use lri_rs::{AwbGain, LriFile, RawImage, Whitepoint};
use nalgebra::{Matrix3, Matrix3x1};
use png::{BitDepth, ColorType, Encoder};
use rawproc::colorspace::BayerRgb;
use rawproc::image::{Image, RawMetadata};

pub fn export_png(
	img: &RawImage<'_>,
	lri: &LriFile<'_>,
	path: Utf8PathBuf,
	awb_gain: AwbGain,
) -> Result<()> {
	let (black, white) = lri.levels_for(img.sensor);
	let range = (white - black).max(1) as f32;

	eprintln!(
		"{} {:?} [{}:{}] {}x{} {} (levels {black}/{white})",
		img.camera, img.sensor, img.sbro.0, img.sbro.1, img.width, img.height, img.format
	);

	let bayered = img.decode_pixels().context("decode sensor pixels")?;

	let (mut pixels, color_format, channels) = match img.cfa_string() {
		Some(cfa_string) => {
			let rawimg: Image<u16, BayerRgb> = Image::from_raw_parts(
				img.width,
				img.height,
				RawMetadata {
					whitebalance: [1.0; 3],
					whitelevels: [white; 3],
					crop: None,
					cfa: rawloader::CFA::new(cfa_string),
					cam_to_xyz: Matrix3::zeros(),
				},
				bayered,
			);

			(rawimg.debayer().data, ColorType::Rgb, 3usize)
		}
		None => (bayered, ColorType::Grayscale, 1usize),
	};

	rotate_180(&mut pixels, channels);

	let mut floats: Vec<f32> = pixels
		.iter()
		.map(|p| (*p).saturating_sub(black) as f32 / range)
		.collect();

	if !img.color.is_empty() {
		eprint!("  whitepoints:");
		for c in &img.color {
			eprint!(" {:?}", c.whitepoint);
		}
		eprintln!();
	}

	match img.color_info(Whitepoint::D65) {
		Some(c) => {
			eprintln!("  colour: D65 forward matrix");
			let to_xyz = Matrix3::from_row_slice(&c.forward_matrix);
			let to_srgb = Matrix3::from_row_slice(&BRUCE_XYZ_RGB_D50);
			let premul = to_xyz * to_srgb;

			for chnk in floats.chunks_mut(3) {
				let r = chnk[0] * awb_gain.r;
				let g = chnk[1];
				let b = chnk[2] * awb_gain.b;
				let rgb = premul * Matrix3x1::new(r, g, b);
				chnk[0] = srgb_gamma(rgb[0]);
				chnk[1] = srgb_gamma(rgb[1]);
				chnk[2] = srgb_gamma(rgb[2]);
			}
		}
		None => {
			eprintln!("  colour: gamma only (no D65 profile)");
			floats.iter_mut().for_each(|f| *f = srgb_gamma(*f));
		}
	}

	let bytes: Vec<u8> = floats.into_iter().map(|f| (f * 255.0) as u8).collect();

	eprintln!("  write {path}");
	write_png(&path, img.width, img.height, &bytes, color_format)
}

fn rotate_180<T: Copy>(data: &mut [T], channels: usize) {
	if channels == 1 {
		data.reverse();
		return;
	}

	let pixels = data.len() / channels;
	let mut tmp = vec![data[0]; data.len()];
	for (dst, src) in (0..pixels).map(|i| i * channels).zip((0..pixels).rev().map(|i| i * channels)) {
		for c in 0..channels {
			tmp[dst + c] = data[src + c];
		}
	}
	data.copy_from_slice(&tmp);
}

#[rustfmt::skip]
const BRUCE_XYZ_RGB_D50: [f32; 9] = [
	3.1338561,  -1.6168667, -0.4906146,
	-0.9787684,  1.9161415,  0.0334540,
	0.0719453,  -0.2289914,  1.4052427
];

#[inline]
fn srgb_gamma(mut float: f32) -> f32 {
	if float <= 0.0031308 {
		float *= 12.92;
	} else {
		float = float.powf(1.0 / 2.4) * 1.055 - 0.055;
	}
	float.clamp(0.0, 1.0)
}

fn write_png(
	path: &Utf8PathBuf,
	width: usize,
	height: usize,
	data: &[u8],
	color_format: ColorType,
) -> Result<()> {
	let file = File::create(path).with_context(|| format!("create {path}"))?;
	let mut enc = Encoder::new(file, width as u32, height as u32);
	enc.set_color(color_format);
	enc.set_depth(BitDepth::Eight);
	let mut writer = enc.write_header().context("png header")?;
	writer.write_image_data(data).context("png data")?;
	Ok(())
}