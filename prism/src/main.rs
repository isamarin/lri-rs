use std::collections::HashMap;

use camino::Utf8PathBuf;
use lri_rs::{AwbGain, CameraId, LriFile, RawData, RawImage, SensorModel, Whitepoint};
use nalgebra::{Matrix3, Matrix3x1};

mod output;
mod rotate;

pub struct Entry {
	sensor: SensorModel,
	count: usize,
}

fn main() {
	let args = std::env::args().skip(1);

	if args.len() != 2 {
		eprintln!("Usage: prism <lri_file> <output_directory>");
		std::process::exit(1);
	}

	let file_name = std::env::args().nth(1).unwrap();
	let directory = Utf8PathBuf::from(std::env::args().nth(2).unwrap());

	if !directory.exists() {
		std::fs::create_dir_all(&directory).unwrap();
	}

	let bytes = std::fs::read(file_name).unwrap();
	let lri = LriFile::decode(&bytes);
	let gain = lri.awb_gain.unwrap();

	println!("{} images", lri.image_count());

	if let Some(refimg) = lri.reference_image() {
		println!("The reference camera is {}", refimg.camera);
	}

	let mut set: HashMap<CameraId, Entry> = HashMap::new();

	for img in lri.images() {
		set.entry(img.camera)
			.and_modify(|e| e.count += 1)
			.or_insert(Entry {
				sensor: img.sensor,
				count: 1,
			});
	}

	/*set.into_iter().for_each(|kv| {
		println!("{} {:?} {}", kv.0, kv.1.sensor, kv.1.count);
	});*/

	for img in lri.images() {
		let path = format!("{}.png", img.camera);
		output::make_png(img, directory.join(path), gain);
	}
}
