use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use lri_rs::{CameraId, LriFile, RawImage};
use nalgebra::Matrix3;
use png::{BitDepth, ColorType, Encoder};
use rawproc::colorspace::BayerRgb;
use rawproc::image::{Image, RawMetadata};

use crate::render::rotate_180;

const MAX_SIDE: u32 = 160;

pub fn thumbnail_base64(lri: &LriFile<'_>, camera: CameraId) -> Result<String> {
	let img = lri
		.images
		.iter()
		.find(|i| i.camera == camera)
		.context("camera not in file")?;

	let png = render_thumbnail(img, lri)?;
	Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

pub fn parse_camera_id(name: &str) -> Option<CameraId> {
	use CameraId::*;
	match name {
		"A1" => Some(A1),
		"A2" => Some(A2),
		"A3" => Some(A3),
		"A4" => Some(A4),
		"A5" => Some(A5),
		"B1" => Some(B1),
		"B2" => Some(B2),
		"B3" => Some(B3),
		"B4" => Some(B4),
		"B5" => Some(B5),
		"C1" => Some(C1),
		"C2" => Some(C2),
		"C3" => Some(C3),
		"C4" => Some(C4),
		"C5" => Some(C5),
		"C6" => Some(C6),
		_ => None,
	}
}

fn render_thumbnail(img: &RawImage<'_>, lri: &LriFile<'_>) -> Result<Vec<u8>> {
	let (black, white) = lri.levels_for(img.sensor);
	let range = (white - black).max(1) as f32;

	let bayer = img.decode_pixels().context("decode")?;
	let step = subsample_step(img.width, img.height, MAX_SIDE);
	let (mut bayer, sw, sh) = subsample(&bayer, img.width, img.height, step);
	rotate_180(&mut bayer, 1);

	let bytes = match img.cfa_string() {
		Some(cfa) => {
			let rawimg: Image<u16, BayerRgb> = Image::from_raw_parts(
				sw,
				sh,
				RawMetadata {
					whitebalance: [1.0; 3],
					whitelevels: [white; 3],
					crop: None,
					cfa: rawloader::CFA::new(cfa),
					cam_to_xyz: Matrix3::zeros(),
				},
				bayer,
			);
			let mut rgb = rawimg.debayer().data;
			rotate_180(&mut rgb, 3);
			rgb.chunks(3)
				.flat_map(|px| {
					px.iter().map(|p| {
						let n = (*p).saturating_sub(black) as f32 / range;
						(n * 255.0).clamp(0.0, 255.0) as u8
					})
				})
				.collect::<Vec<u8>>()
		}
		None => bayer
			.iter()
			.map(|p| {
				let n = (*p).saturating_sub(black) as f32 / range;
				(n * 255.0).clamp(0.0, 255.0) as u8
			})
			.collect(),
	};

	let (w, h, color) = match img.cfa_string() {
		Some(_) => (sw as u32, sh as u32, ColorType::Rgb),
		None => (sw as u32, sh as u32, ColorType::Grayscale),
	};

	encode_png(w, h, &bytes, color)
}

fn subsample_step(width: usize, height: usize, max_side: u32) -> usize {
	let max_dim = width.max(height) as u32;
	((max_dim + max_side - 1) / max_side).max(1) as usize
}

fn subsample(data: &[u16], width: usize, height: usize, step: usize) -> (Vec<u16>, usize, usize) {
	let sw = width.div_ceil(step);
	let sh = height.div_ceil(step);
	let mut out = Vec::with_capacity(sw * sh);
	for y in (0..height).step_by(step) {
		for x in (0..width).step_by(step) {
			out.push(data[y * width + x]);
		}
	}
	(out, sw, sh)
}

fn encode_png(width: u32, height: u32, data: &[u8], color: ColorType) -> Result<Vec<u8>> {
	let mut buf = Vec::new();
	{
		let mut enc = Encoder::new(&mut buf, width, height);
		enc.set_color(color);
		enc.set_depth(BitDepth::Eight);
		let mut writer = enc.write_header().context("png header")?;
		writer.write_image_data(data).context("png data")?;
	}
	Ok(buf)
}