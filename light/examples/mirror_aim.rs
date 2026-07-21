//! Does each movable mirror point where the camera says it can point?
//!
//! For every mirror module this prints the per-shot Hall code, the angle our
//! port derives from it, and the angle range the calibration data itself
//! declares (`MirrorSystem.mirror_angle_range`). An angle outside that range is
//! not a tuning matter — the camera is telling us the mirror cannot physically
//! be there, so either the Hall→angle mapping or the range is being read wrong.
//!
//! This is the same kind of check as `det(R) = +1`: it can only come out right
//! or wrong, and it does not depend on image content. Worth exhausting before
//! any NCC-based reasoning about the C row, whose overlap with the reference is
//! ~20 % of canvas and whose pointing is the thing in question.
//!
//! ```text
//! cargo run -p light --example mirror_aim -- .data-from-camera/raw/L16_00003.lri
//! ```

use lri_rs::LriFile;
use std::collections::HashSet;
use std::fs;

fn main() {
	let args: Vec<String> = std::env::args().skip(1).collect();
	if args.is_empty() {
		eprintln!("usage: mirror_aim <file.lri> [more.lri ...]");
		std::process::exit(2);
	}
	let mut out_of_range = 0usize;
	for p in &args {
		match dump(p) {
			Ok(n) => out_of_range += n,
			Err(e) => eprintln!("{p}: {e}"),
		}
		println!();
	}
	if out_of_range > 0 {
		println!("{out_of_range} module-shots outside the declared mirror range");
	}
}

fn dump(path: &str) -> Result<usize, String> {
	let data = fs::read(path).map_err(|e| e.to_string())?;
	let lri = LriFile::decode(&data).map_err(|e| format!("{e:?}"))?;
	let focal = lri.focal_length.ok_or("missing focal length")?;
	let fired: HashSet<_> = lri.images.iter().map(|i| i.camera).collect();

	println!("── {path}");
	println!("   focal {focal}mm · reference {:?}", lri.image_reference_camera);
	println!();
	println!(
		"   {:<8} {:<6} {:>6} {:>9} {:>16} {:>8}  {}",
		"camera", "fired", "hall", "angle°", "declared range°", "margin", "verdict"
	);

	let mut bad = 0usize;
	for module in &lri.fusion.module_geometry {
		let cam = module.camera;
		let Some(mm) = module
			.focus_calibrations
			.iter()
			.find_map(|f| f.movable_mirror.as_ref())
		else {
			continue;
		};
		let (Some(ms), Some(mapping)) = (mm.mirror_system.as_ref(), mm.actuator_mapping.as_ref())
		else {
			continue;
		};

		let hall = lri.fusion.mirror_hall_codes.get(&cam).copied();
		let angle = hall.and_then(|h| lri_rs::hall_code_to_mirror_angle_deg(h, mapping));
		let range = &ms.mirror_angle_range;

		let (angle_s, margin_s, verdict) = match angle {
			Some(a) => {
				// Negative margin = how far outside the range the mirror is claimed to be.
				let margin = (a - range.min as f64).min(range.max as f64 - a);
				let verdict = if margin >= 0.0 {
					"in range".to_string()
				} else {
					bad += 1;
					format!("OUT OF RANGE by {:.2}°", -margin)
				};
				(format!("{a:+9.3}"), format!("{margin:+8.2}"), verdict)
			}
			None => ("        —".into(), "       —".into(), "no hall code".into()),
		};

		println!(
			"   {:<8} {:<6} {:>6} {} {:>16} {}  {}",
			format!("{cam:?}"),
			if fired.contains(&cam) { "yes" } else { "no" },
			hall.map(|h| h.to_string()).unwrap_or_else(|| "—".into()),
			angle_s,
			format!("{:.2}..{:.2}", range.min, range.max),
			margin_s,
			verdict
		);
	}
	pointing(&lri, focal, &fired);
	intrinsics(&lri, focal);
	parallax(&lri, focal, 832);
	layout(&lri, focal);
	Ok(bad)
}

/// Where each module's optical axis actually points, relative to the reference.
///
/// With `x_cam = R·X + t`, the viewing direction in world coordinates is
/// `Rᵀ·ẑ`. The angle between a module's axis and the reference's is a pure
/// geometry number — no image content, so it is meaningful for the C row even
/// though NCC against B4 is not.
///
/// Scale to judge it against: the modules have to tile a shared field of view.
/// A few degrees of offset is the design. Tens of degrees means the module is
/// aimed somewhere the reference cannot see, and no warp will rescue it.
fn pointing(lri: &LriFile<'_>, focal: i32, fired: &HashSet<lri_rs::CameraId>) {
	let Some(ref_cam) = lri.image_reference_camera else {
		return;
	};
	let picks = lri.fusion.pick_all_focus_bundles(focal);
	let axis_of = |r: &[f32; 9]| {
		// Rᵀ·ẑ — third *row* of R read as a column.
		[r[6] as f64, r[7] as f64, r[8] as f64]
	};
	let Some(ref_axis) = picks
		.iter()
		.find(|(c, _)| *c == ref_cam)
		.and_then(|(_, s)| s.rotation.as_ref().map(axis_of))
	else {
		return;
	};

	println!();
	println!(
		"   {:<8} {:<6} {:<7} {:>10} {:>9} {:>9}  {}",
		"camera", "fired", "path", "off-axis°", "yaw°", "pitch°", "vs reference"
	);
	for (cam, sel) in &picks {
		let Some(r) = sel.rotation.as_ref() else {
			continue;
		};
		let a = axis_of(r);
		let dot = (a[0] * ref_axis[0] + a[1] * ref_axis[1] + a[2] * ref_axis[2]).clamp(-1.0, 1.0);
		let off = dot.acos().to_degrees();
		// Decompose so a systematic aim error is readable, not just its magnitude.
		let yaw = (a[0].atan2(a[2]) - ref_axis[0].atan2(ref_axis[2])).to_degrees();
		let pitch = (a[1].asin() - ref_axis[1].asin()).to_degrees();
		let verdict = if *cam == ref_cam {
			"reference".to_string()
		} else if off > 20.0 {
			format!("AIMED AWAY — {off:.0}° off")
		} else if off > 5.0 {
			"wide".to_string()
		} else {
			"plausible".to_string()
		};
		println!(
			"   {:<8} {:<6} {:<7} {:>10.2} {:>9.2} {:>9.2}  {}",
			format!("{cam:?}"),
			if fired.contains(cam) { "yes" } else { "no" },
			if sel.has_movable_mirror { "mirror" } else { "canon" },
			off,
			yaw,
			pitch,
			verdict
		);
	}
}

/// Intrinsics actually handed to the warp, per module.
///
/// The warp uses K and R only. Pointing is now known to be within a few degrees
/// for every module, so if a module still fails to align, K is the remaining
/// suspect — and `pick_extrinsics_bundle` takes no focal length while
/// `pick_intrinsics_bundle` does, so the two halves of a pose can come from
/// different focus planes (OPEN-QUESTIONS §1b).
fn intrinsics(lri: &LriFile<'_>, focal: i32) {
	println!();
	println!(
		"   {:<8} {:<7} {:>8} {:>8} {:>9} {:>9} {:>7} {:>7}",
		"camera", "path", "fx", "fy", "cx", "cy", "int.fd", "ext.fd"
	);
	for (cam, sel) in &lri.fusion.pick_all_focus_bundles(focal) {
		let Some(k) = sel.k_matrix else { continue };
		println!(
			"   {:<8} {:<7} {:>8.1} {:>8.1} {:>9.1} {:>9.1} {:>7.0} {:>7}",
			format!("{cam:?}"),
			if sel.has_movable_mirror { "mirror" } else { "canon" },
			k[0],
			k[4],
			k[2],
			k[5],
			sel.intrinsics_focus_distance,
			sel.extrinsics_focus_distance
				.map(|v| format!("{v:.0}"))
				.unwrap_or_else(|| "—".into()),
		);
	}
}

/// Do the sixteen camera centres form the physical front face of an L16?
///
/// This is the one check that needs no images at all, which matters because the
/// A row and the glued C modules **never fire in the same shot** — the camera
/// picks wide (A + B, reference A1) or tele (B + C, reference B4) and there is no
/// mode that mixes them. No capture can compare those two reference classes by
/// correlation, so the comparison has to be algebraic.
///
/// The sixteen modules are mounted on one flat face in a fixed grid. So their
/// centres, `C = −Rᵀ·t`, must be near-coplanar and laid out in rows. A module
/// whose pose is stored in a different convention lands somewhere that
/// arrangement does not allow — and it says so without reference to any image.
///
/// Reported as distance from the best-fit plane through all centres, then as
/// in-plane coordinates so the row structure is readable.
fn layout(lri: &LriFile<'_>, focal: i32) {
	let picks = lri.fusion.pick_all_focus_bundles(focal);
	let mut pts: Vec<(String, &'static str, [f64; 3])> = Vec::new();
	for (cam, sel) in &picks {
		let (Some(r), Some(t)) = (sel.rotation, sel.translation) else {
			continue;
		};
		let (r, t) = (r.map(f64::from), t.map(f64::from));
		let c = [
			-(r[0] * t[0] + r[3] * t[1] + r[6] * t[2]),
			-(r[1] * t[0] + r[4] * t[1] + r[7] * t[2]),
			-(r[2] * t[0] + r[5] * t[1] + r[8] * t[2]),
		];
		let path = if sel.has_movable_mirror { "mirror" } else { "canon" };
		pts.push((format!("{cam:?}"), path, c));
	}
	if pts.len() < 4 {
		return;
	}

	let n = pts.len() as f64;
	let mut mean = [0.0; 3];
	for (_, _, c) in &pts {
		for i in 0..3 {
			mean[i] += c[i] / n;
		}
	}
	// Plane normal = eigenvector of the smallest eigenvalue of the scatter matrix.
	// Power-iterate on (trace·I − M), whose dominant eigenvector is M's smallest.
	let mut m = [[0.0f64; 3]; 3];
	for (_, _, c) in &pts {
		let d = [c[0] - mean[0], c[1] - mean[1], c[2] - mean[2]];
		for i in 0..3 {
			for j in 0..3 {
				m[i][j] += d[i] * d[j];
			}
		}
	}
	let trace = m[0][0] + m[1][1] + m[2][2];
	let mut b = [[0.0f64; 3]; 3];
	for i in 0..3 {
		for j in 0..3 {
			b[i][j] = if i == j { trace - m[i][j] } else { -m[i][j] };
		}
	}
	let mut v = [1.0f64, 0.3, 0.7];
	for _ in 0..200 {
		let w = [
			b[0][0] * v[0] + b[0][1] * v[1] + b[0][2] * v[2],
			b[1][0] * v[0] + b[1][1] * v[1] + b[1][2] * v[2],
			b[2][0] * v[0] + b[2][1] * v[1] + b[2][2] * v[2],
		];
		let len = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
		if len < 1e-12 {
			break;
		}
		v = [w[0] / len, w[1] / len, w[2] / len];
	}

	// One plane through all sixteen is the wrong model, and the data says so
	// immediately: the rows sit at distinct depths. That is physics, not a bug —
	// a folded optical path puts the virtual centre behind the face, so the
	// mirror rows are displaced backwards from the flat-mounted A row. What must
	// hold is consistency *within* a row.
	let _ = (mean, v);
	let mut by_row: std::collections::BTreeMap<char, Vec<&(String, &'static str, [f64; 3])>> =
		Default::default();
	for p in &pts {
		by_row.entry(p.0.chars().next().unwrap_or('?')).or_default().push(p);
	}

	println!();
	println!("   front-face layout — centres C = −Rᵀ·t, mm (origin is A1)");
	println!(
		"   {:<8} {:<7} {:>9} {:>9} {:>9} {:>11}  {}",
		"camera", "path", "x", "y", "z", "z vs row", "verdict"
	);
	for (row, group) in &by_row {
		let mean_z: f64 = group.iter().map(|(_, _, c)| c[2]).sum::<f64>() / group.len() as f64;
		for (cam, path, c) in group {
			let dz = c[2] - mean_z;
			// Modules of one row are mounted on one plate; a pose stored in a
			// foreign convention would not land on it.
			let verdict = if dz.abs() > 3.0 { "OFF ROW PLANE" } else { "consistent" };
			println!(
				"   {:<8} {:<7} {:>9.2} {:>9.2} {:>9.2} {:>11.3}  {}",
				cam, path, c[0], c[1], c[2], dz, verdict
			);
		}
		let spread = group
			.iter()
			.map(|(_, _, c)| c[2])
			.fold(f64::NEG_INFINITY, f64::max)
			- group
				.iter()
				.map(|(_, _, c)| c[2])
				.fold(f64::INFINITY, f64::min);
		println!("   row {row}: mean z {mean_z:+.2} mm, spread {spread:.2} mm");
	}
}

/// Is an infinity homography even applicable to this module?
///
/// `homography_infinity` drops `t` — it models every module as sharing one
/// optical centre. The error that hides is parallax, and parallax in *pixels*
/// scales with focal length: `disparity ≈ fx · baseline / distance`. The same
/// physical baseline that costs the 28 mm row a pixel costs the 150 mm row an
/// order of magnitude more.
///
/// So a C module can have correct parity, correct pointing and correct K, and
/// still refuse to correlate — because the model being applied to it is wrong.
/// Camera centre is `C = −Rᵀ·t`; baseline is `|C − C_ref|`.
fn parallax(lri: &LriFile<'_>, focal: i32, max_side: u32) {
	let Some(ref_cam) = lri.image_reference_camera else {
		return;
	};
	let picks = lri.fusion.pick_all_focus_bundles(focal);
	let centre = |sel: &lri_rs::SelectedFocusBundle| {
		let r = sel.rotation?;
		let t = sel.translation?;
		let (r, t) = (r.map(f64::from), t.map(f64::from));
		// −Rᵀ·t
		Some([
			-(r[0] * t[0] + r[3] * t[1] + r[6] * t[2]),
			-(r[1] * t[0] + r[4] * t[1] + r[7] * t[2]),
			-(r[2] * t[0] + r[5] * t[1] + r[8] * t[2]),
		])
	};
	let Some(ref_c) = picks.iter().find(|(c, _)| *c == ref_cam).and_then(|(_, s)| centre(s)) else {
		return;
	};

	// Scene distance: ToF if the shot has it, else the calibration plane.
	let dist_mm = lri
		.fusion
		.tof_range_m
		.filter(|v| *v > 0.0)
		.map(|v| f64::from(v) * 1000.0)
		.unwrap_or(1500.0);
	let scale = f64::from(max_side) / 4000.0;

	println!();
	println!("   scene distance {dist_mm:.0} mm · preview scale {scale:.3}");
	println!(
		"   {:<8} {:<7} {:>10} {:>13} {:>13}  {}",
		"camera", "path", "base mm", "disp px full", "disp px prev", "infinity model"
	);
	for (cam, sel) in &picks {
		let (Some(c), Some(k)) = (centre(sel), sel.k_matrix) else {
			continue;
		};
		let base = ((c[0] - ref_c[0]).powi(2) + (c[1] - ref_c[1]).powi(2) + (c[2] - ref_c[2]).powi(2))
			.sqrt();
		let disp = f64::from(k[0]) * base / dist_mm;
		let disp_prev = disp * scale;
		let verdict = if *cam == ref_cam {
			"reference".to_string()
		} else if disp_prev > 20.0 {
			format!("BROKEN — {disp_prev:.0} px of parallax ignored")
		} else if disp_prev > 5.0 {
			"marginal".to_string()
		} else {
			"ok".to_string()
		};
		println!(
			"   {:<8} {:<7} {:>10.2} {:>13.1} {:>13.1}  {}",
			format!("{cam:?}"),
			if sel.has_movable_mirror { "mirror" } else { "canon" },
			base,
			disp,
			disp_prev,
			verdict
		);
	}
}
