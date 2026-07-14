use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use camino::Utf8Path;
use lri_rs::LriFile;
use rayon::prelude::*;

use crate::render;
use crate::session::LriSession;
use crate::threads;

pub fn run(input: &Utf8Path, output: &Utf8Path, jobs: Option<usize>) -> Result<()> {
	run_with_progress(input, output, jobs, |_, _, _| {})
}

pub fn run_with_progress(
	input: &Utf8Path,
	output: &Utf8Path,
	jobs: Option<usize>,
	on_progress: impl Fn(usize, usize, &str) + Send + Sync + 'static,
) -> Result<()> {
	let session = LriSession::open(input)?;
	run_session_with_progress(&session, output, jobs, on_progress)
}

pub fn run_session_with_progress(
	session: &LriSession,
	output: &Utf8Path,
	jobs: Option<usize>,
	on_progress: impl Fn(usize, usize, &str) + Send + Sync + 'static,
) -> Result<()> {
	let n = threads::export_jobs(jobs);
	let pool = rayon::ThreadPoolBuilder::new()
		.num_threads(n)
		.build()
		.context("configure thread pool")?;

	session.with_lri(|lri| {
		pool.install(|| run_decoded(lri, output, on_progress))
	})
}

fn run_decoded(
	lri: &LriFile<'_>,
	output: &Utf8Path,
	on_progress: impl Fn(usize, usize, &str) + Send + Sync + 'static,
) -> Result<()> {
	if !output.exists() {
		std::fs::create_dir_all(output).context("create output directory")?;
	}

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