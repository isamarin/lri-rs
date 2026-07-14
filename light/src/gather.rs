use std::{
	collections::HashMap,
	time::{Duration, Instant},
};

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use lri_rs::{AwbMode, DataFormat, HdrMode, LriFile, SceneMode, SensorModel};
use owo_colors::OwoColorize;

pub fn run(data_dir: &Utf8Path) -> Result<()> {
	let mut files: HashMap<String, Photo> = HashMap::new();

	for entry in data_dir.read_dir_utf8().context("read directory")? {
		let entry = entry.context("read dir entry")?;
		let meta = entry.metadata().context("stat entry")?;
		let path = entry.path();

		if !meta.is_file() {
			continue;
		}

		let stub = path
			.file_stem()
			.context("missing file stem")?
			.to_owned();

		match path.extension() {
			Some("jpg") => {
				files
					.entry(stub.clone())
					.and_modify(|e| e.jpg = Some(path.to_owned()))
					.or_insert_with(|| Photo::new_jpg(path));
			}
			Some("lri") => {
				files
					.entry(stub.clone())
					.and_modify(|e| e.lri = Some(path.to_owned()))
					.or_insert_with(|| Photo::new_lri(path));
			}
			Some("lris") => {
				files
					.entry(stub.clone())
					.and_modify(|e| e.lris = Some(path.to_owned()))
					.or_insert_with(|| Photo::new_lris(path));
			}
			_ => {}
		}
	}

	let start = Instant::now();
	let mut photos: Vec<Photo> = files.into_values().collect();
	photos.sort_by(|a, b| a.lri.as_deref().cmp(&b.lri.as_deref()));

	for photo in photos {
		let Some(lri_path) = photo.lri else {
			continue;
		};

		let data = match std::fs::read(&lri_path) {
			Ok(d) => d,
			Err(e) => {
				eprintln!("{}: {e}", lri_path.red());
				continue;
			}
		};

		let lri = match LriFile::decode(&data) {
			Ok(l) => l,
			Err(e) => {
				eprintln!("{}: {e}", lri_path.red());
				continue;
			}
		};

		print!("{} - ", lri_path.file_stem().unwrap_or_default());

		if let Some(fwv) = lri.firmware_version.as_ref() {
			print!(
				"[{fwv}] focal:{:<3} iit:{:>2}ms gain:{:2.0} ",
				lri.focal_length.unwrap_or_default(),
				lri.image_integration_time
					.unwrap_or(Duration::ZERO)
					.as_millis(),
				lri.image_gain.unwrap_or_default()
			);

			match lri.hdr {
				None => print!("hdr:{} ", "non".dimmed()),
				Some(HdrMode::None) => print!("hdr:{} ", "nop".dimmed()),
				Some(HdrMode::Default) => print!("hdr:hdr "),
				Some(HdrMode::Natural) => print!("hdr:{} ", "nat".bright_green()),
				Some(HdrMode::Surreal) => print!("hdr:{} ", "sur".bright_magenta()),
			}

			match lri.scene {
				None => print!("sc:{} ", "non".dimmed()),
				Some(SceneMode::None) => print!("sc:{} ", "nop".dimmed()),
				Some(SceneMode::Portrait) => print!("sc:prt "),
				Some(SceneMode::Landscape) => print!("sc:lnd "),
				Some(SceneMode::Macro) => print!("sc:mcr "),
				Some(SceneMode::Sport) => print!("sc:spt "),
				Some(SceneMode::Night) => print!("sc:ni  "),
			}

			match lri.on_tripod {
				None => print!("{} ", "tri".dimmed()),
				Some(false) => print!("{} ", "tri".red()),
				Some(true) => print!("{} ", "tri".green()),
			}

			match lri.af_achieved {
				None => print!("{} - ", "af".dimmed()),
				Some(false) => print!("{} - ", "af".red()),
				Some(true) => print!("{} - ", "af".green()),
			}

			match lri.awb {
				None => print!("{}:", "awb".dimmed()),
				Some(AwbMode::Auto) => print!("{}:", "awb".white()),
				Some(AwbMode::Daylight) => print!("{}:", "awb".yellow()),
			}

			match lri.awb_gain {
				None => print!("{} - ", "gain".dimmed()),
				Some(gain) => print!(
					"{} - [{:.2},{:.2},{:.2},{:.2}] ",
					"gain".white(),
					gain.r,
					gain.gr,
					gain.gb,
					gain.b
				),
			}
		}

		for img in lri.images() {
			let sens = match img.sensor {
				SensorModel::Ar1335 => "a13",
				SensorModel::Ar1335Mono => "a1m",
				SensorModel::Unknown => "???",
			};

			match img.format {
				DataFormat::BayerJpeg => print!("{} ", sens.cyan()),
				DataFormat::Packed10bpp => print!("{} ", sens.yellow()),
			}
		}
		println!();
	}

	eprintln!("        ---\nTook {:.2}s", start.elapsed().as_secs_f32());
	Ok(())
}

struct Photo {
	jpg: Option<Utf8PathBuf>,
	lri: Option<Utf8PathBuf>,
	lris: Option<Utf8PathBuf>,
}

impl Photo {
	fn new_jpg(jpg: &Utf8Path) -> Self {
		Self {
			jpg: Some(jpg.to_owned()),
			lri: None,
			lris: None,
		}
	}

	fn new_lri(lri: &Utf8Path) -> Self {
		Self {
			lri: Some(lri.to_owned()),
			jpg: None,
			lris: None,
		}
	}

	fn new_lris(lris: &Utf8Path) -> Self {
		Self {
			lris: Some(lris.to_owned()),
			lri: None,
			jpg: None,
		}
	}
}