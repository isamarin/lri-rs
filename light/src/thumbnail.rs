use std::collections::HashMap;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use lri_rs::{CameraId, LriFile, RawImage};
use png::{BitDepth, ColorType, Encoder};
use rayon::prelude::*;

use crate::render::rotate_180;
use crate::threads;

const MAX_SIDE: u32 = 160;

pub fn thumbnail_base64(lri: &LriFile<'_>, camera: CameraId) -> Result<String> {
	let img = lri
		.images
		.iter()
		.find(|i| i.camera == camera)
		.context("camera not in file")?;

	let png = render_thumbnail_fast(img, lri)?;
	Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

pub fn thumbnails_batch(
	lri: &LriFile<'_>,
	cameras: &[CameraId],
	jobs: Option<usize>,
) -> Result<HashMap<String, String>> {
	let n = threads::export_jobs(jobs);
	let pool = rayon::ThreadPoolBuilder::new()
		.num_threads(n)
		.build()
		.context("thumbnail thread pool")?;

	pool.install(|| {
		cameras
			.par_iter()
			.map(|&camera| {
				let data_url = thumbnail_base64(lri, camera)?;
				Ok((camera.to_string(), data_url))
			})
			.collect::<Result<HashMap<_, _>>>()
	})
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

/// Grid preview: subsampled Bayer as grayscale — no debayer.
fn render_thumbnail_fast(img: &RawImage<'_>, lri: &LriFile<'_>) -> Result<Vec<u8>> {
	let (black, white) = lri.levels_for(img.sensor);
	let range = (white - black).max(1) as f32;

	let preview = img.decode_preview().context("decode preview")?;
	let step = subsample_step(preview.width, preview.height, MAX_SIDE);
	let (mut bayer, sw, sh) = subsample(&preview.data, preview.width, preview.height, step);
	rotate_180(&mut bayer, 1);

	let bytes: Vec<u8> = bayer
		.iter()
		.map(|p| {
			let n = (*p).saturating_sub(black) as f32 / range;
			(n * 255.0).clamp(0.0, 255.0) as u8
		})
		.collect();

	encode_png(sw as u32, sh as u32, &bytes, ColorType::Grayscale)
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