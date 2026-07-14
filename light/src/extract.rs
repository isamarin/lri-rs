use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use camino::Utf8Path;
use lri_rs::LriFile;
use rayon::prelude::*;

use crate::render;

pub fn run(input: &Utf8Path, output: &Utf8Path, jobs: Option<usize>) -> Result<()> {
	run_with_progress(input, output, jobs, |_, _, _| {})
}

pub fn run_with_progress(
	input: &Utf8Path,
	output: &Utf8Path,
	jobs: Option<usize>,
	on_progress: impl Fn(usize, usize, &str) + Send + Sync + 'static,
) -> Result<()> {
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

	let total = lri.image_count();
	let done = AtomicUsize::new(0);
	let on_progress = Arc::new(on_progress);

	eprintln!("{total} images");

	if let Some(refimg) = lri.reference_image() {
		eprintln!("reference camera: {}", refimg.camera);
	}

	lri.images.par_iter().try_for_each(|img| {
		let path = output.join(format!("{}.dng", img.camera));
		render::export_dng(img, &lri, path)?;

		let n = done.fetch_add(1, Ordering::SeqCst) + 1;
		on_progress(n, total, &img.camera.to_string());
		Ok::<(), anyhow::Error>(())
	})?;

	Ok(())
}