use std::sync::Arc;

use anyhow::{Context, Result};
use camino::Utf8Path;
use lri_rs::{AwbGain, LriFile};
use rayon::prelude::*;

use crate::render;

pub fn run(input: &Utf8Path, output: &Utf8Path, jobs: Option<usize>) -> Result<()> {
	if let Some(n) = jobs {
		rayon::ThreadPoolBuilder::new()
			.num_threads(n)
			.build_global()
			.context("configure thread pool")?;
	}

	if !output.exists() {
		std::fs::create_dir_all(output).context("create output directory")?;
	}

	let bytes = std::fs::read(input).with_context(|| format!("read {}", input))?;
	let lri = LriFile::decode(&bytes).context("decode LRI")?;
	let lri = Arc::new(lri);

	let gain = lri.awb_gain.unwrap_or(AwbGain {
		r: 1.0,
		gr: 1.0,
		gb: 1.0,
		b: 1.0,
	});

	eprintln!("{} images", lri.image_count());

	if let Some(refimg) = lri.reference_image() {
		eprintln!("reference camera: {}", refimg.camera);
	}

	let images: Vec<_> = lri.images.iter().collect();

	images.par_iter().try_for_each(|img| {
		let path = output.join(format!("{}.png", img.camera));
		render::export_png(img, &lri, path, gain)
	})?;

	Ok(())
}