use std::path::Path;

use anyhow::{Context, Result};
use lri_rs::{AwbMode, DataFormat, HdrMode, LriFile, SceneMode, SensorModel};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LriSummary {
	pub path: String,
	pub name: String,
	pub firmware: Option<String>,
	pub focal_length: Option<i32>,
	pub integration_ms: Option<u64>,
	pub gain: Option<f32>,
	pub hdr: Option<String>,
	pub scene: Option<String>,
	pub on_tripod: Option<bool>,
	pub af_achieved: Option<bool>,
	pub awb: Option<String>,
	pub awb_gain: Option<[f32; 4]>,
	pub reference_camera: Option<String>,
	pub image_count: usize,
	pub cameras: Vec<CameraSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CameraSummary {
	pub id: String,
	pub sensor: String,
	pub format: String,
	pub width: usize,
	pub height: usize,
	pub bayer_jpeg: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirScan {
	pub directory: String,
	pub files: Vec<LriSummary>,
}

pub fn inspect_file(path: impl AsRef<Path>) -> Result<LriSummary> {
	let path = path.as_ref();
	let data = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
	let lri = LriFile::decode(&data).context("decode LRI")?;
	let name = path
		.file_stem()
		.map(|s| s.to_string_lossy().into_owned())
		.unwrap_or_default();

	Ok(summarize(
		path.to_string_lossy().into_owned(),
		name,
		&lri,
	))
}

pub fn scan_directory(path: impl AsRef<Path>) -> Result<DirScan> {
	let path = path.as_ref();
	let mut files = Vec::new();

	for entry in std::fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
		let entry = entry?;
		let meta = entry.metadata()?;
		if !meta.is_file() {
			continue;
		}
		let p = entry.path();
		if p.extension().and_then(|e| e.to_str()) != Some("lri") {
			continue;
		}
		match inspect_file(&p) {
			Ok(summary) => files.push(summary),
			Err(e) => eprintln!("{}: {e}", p.display()),
		}
	}

	files.sort_by(|a, b| a.name.cmp(&b.name));

	Ok(DirScan {
		directory: path.to_string_lossy().into_owned(),
		files,
	})
}

fn summarize(path: String, name: String, lri: &LriFile<'_>) -> LriSummary {
	LriSummary {
		path,
		name,
		firmware: lri.firmware_version.clone(),
		focal_length: lri.focal_length,
		integration_ms: lri
			.image_integration_time
			.map(|d| d.as_millis() as u64),
		gain: lri.image_gain,
		hdr: lri.hdr.map(hdr_label),
		scene: lri.scene.map(scene_label),
		on_tripod: lri.on_tripod,
		af_achieved: lri.af_achieved,
		awb: lri.awb.map(awb_label),
		awb_gain: lri.awb_gain.map(|g| [g.r, g.gr, g.gb, g.b]),
		reference_camera: lri
			.image_reference_camera
			.map(|c| c.to_string()),
		image_count: lri.image_count(),
		cameras: lri.images().map(camera_summary).collect(),
	}
}

fn camera_summary(img: &lri_rs::RawImage<'_>) -> CameraSummary {
	CameraSummary {
		id: img.camera.to_string(),
		sensor: sensor_label(img.sensor),
		format: img.format.to_string(),
		width: img.width,
		height: img.height,
		bayer_jpeg: matches!(img.format, DataFormat::BayerJpeg),
	}
}

fn sensor_label(sensor: SensorModel) -> String {
	match sensor {
		SensorModel::Ar1335 => "AR1335".into(),
		SensorModel::Ar1335Mono => "AR1335 Mono".into(),
		SensorModel::Unknown => "Unknown".into(),
	}
}

fn hdr_label(mode: HdrMode) -> String {
	match mode {
		HdrMode::None => "none".into(),
		HdrMode::Default => "default".into(),
		HdrMode::Natural => "natural".into(),
		HdrMode::Surreal => "surreal".into(),
	}
}

fn scene_label(mode: SceneMode) -> String {
	match mode {
		SceneMode::Portrait => "portrait".into(),
		SceneMode::Landscape => "landscape".into(),
		SceneMode::Sport => "sport".into(),
		SceneMode::Macro => "macro".into(),
		SceneMode::Night => "night".into(),
		SceneMode::None => "none".into(),
	}
}

fn awb_label(mode: AwbMode) -> String {
	match mode {
		AwbMode::Auto => "auto".into(),
		AwbMode::Daylight => "daylight".into(),
	}
}