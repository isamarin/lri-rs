//! Parity check across all modules of a capture.
//!
//! Prints the determinant of the effective rotation each module ends up with,
//! next to its mirror class and which code path produced the pose. `det = +1` is
//! a proper rotation; `det = −1` means a reflection survived uncorrected and the
//! module sits in a mirrored frame — which reads as *negative* correlation
//! against a correctly-handed reference, not merely as a miss.
//!
//! ```text
//! cargo run -p light --example pose_det -- .data-from-camera/raw/L16_00003.lri
//! ```

use std::collections::HashSet;
use std::fs;

use lri_rs::{rotation_determinant, LriFile};

fn main() {
	let mut args = std::env::args().skip(1);
	let path = match args.next() {
		Some(p) => p,
		None => {
			eprintln!("usage: pose_det <file.lri> [more.lri ...]");
			std::process::exit(2);
		}
	};
	let rest: Vec<String> = args.collect();

	for p in std::iter::once(path).chain(rest) {
		if let Err(e) = dump(&p) {
			eprintln!("{p}: {e}");
		}
		println!();
	}
}

fn dump(path: &str) -> Result<(), String> {
	let data = fs::read(path).map_err(|e| e.to_string())?;
	let lri = LriFile::decode(&data).map_err(|e| format!("{e:?}"))?;

	let focal = lri.focal_length.ok_or("missing focal length")?;
	let reference = lri.image_reference_camera;
	let fired: HashSet<_> = lri.images.iter().map(|i| i.camera).collect();

	println!("── {path}");
	println!("   focal {focal}mm · reference {reference:?} · {} modules fired", fired.len());
	println!();
	println!(
		"   {:<8} {:<9} {:<7} {:<6} {:<6} {:>9}  {}",
		"camera", "mirror", "path", "fired", "flip", "det(R)", "parity"
	);

	let picks = lri.fusion.pick_all_focus_bundles(focal);
	let mut improper = Vec::new();
	let mut proper = 0usize;

	for (cam, sel) in &picks {
		let mirror = lri
			.fusion
			.module_geometry
			.iter()
			.find(|m| m.camera == *cam)
			.and_then(|m| m.mirror_type)
			.map(|t| format!("{t:?}"))
			.unwrap_or_else(|| "—".into());

		// Which branch of pick_focus_bundle_with_mirror produced this pose.
		let source = if sel.has_movable_mirror { "mirror" } else { "canon" };

		// The flag the port uses as an image flip — read straight from the file.
		let flip = lri
			.fusion
			.module_geometry
			.iter()
			.find(|m| m.camera == *cam)
			.and_then(|m| {
				m.focus_calibrations
					.iter()
					.find_map(|f| f.movable_mirror.as_ref())
			})
			.and_then(|mm| mm.mirror_system.as_ref())
			.map(|ms| if ms.flip_img_around_x { "true" } else { "false" })
			.unwrap_or("—");

		let (det_s, parity) = match sel.rotation {
			Some(r) => {
				let d = rotation_determinant(&r);
				if (d - 1.0).abs() < 0.05 {
					proper += 1;
					(format!("{d:+.5}"), "proper".to_string())
				} else if (d + 1.0).abs() < 0.05 {
					improper.push(format!("{cam:?}"));
					(format!("{d:+.5}"), "IMPROPER ← mirrored".to_string())
				} else {
					(format!("{d:+.5}"), "not orthonormal".to_string())
				}
			}
			None => ("—".into(), "no extrinsics".into()),
		};

		println!(
			"   {:<8} {:<9} {:<7} {:<6} {:<6} {:>9}  {}",
			format!("{cam:?}"),
			mirror,
			source,
			if fired.contains(cam) { "yes" } else { "no" },
			flip,
			det_s,
			parity
		);
	}

	println!();
	println!("   proper: {proper} · improper: {}", improper.len());
	if !improper.is_empty() {
		println!("   mirrored frame: {}", improper.join(", "));
	}
	Ok(())
}
