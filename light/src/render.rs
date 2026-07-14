use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use lri_rs::{LriFile, RawImage, Whitepoint};

use crate::dng::{self, cfa_pattern};

pub fn export_dng(img: &RawImage<'_>, lri: &LriFile<'_>, path: Utf8PathBuf) -> Result<()> {
	let (black, white) = lri.levels_for(img.sensor);

	eprintln!(
		"{} {:?} [{}:{}] {}x{} {} (levels {black}/{white})",
		img.camera, img.sensor, img.sbro.0, img.sbro.1, img.width, img.height, img.format
	);

	let mut bayer = img.decode_pixels().context("decode sensor pixels")?;
	rotate_180(&mut bayer, 1);

	let cfa = img.cfa_string().and_then(cfa_pattern);
	let color_matrix = img
		.color_info(Whitepoint::D65)
		.map(|c| c.forward_matrix);

	eprintln!("  write {path}");
	dng::write_dng(
		&path,
		img.width as u32,
		img.height as u32,
		&bayer,
		cfa,
		black,
		white,
		color_matrix,
		&img.camera.to_string(),
	)
}

pub fn rotate_180<T: Copy>(data: &mut [T], channels: usize) {
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