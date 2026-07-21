//! What personal data is in these captures, before any of them is published.
//!
//! An `.lri` written by the camera app carries a live GPS fix in a trailing
//! `LELR` block (`FUSION.md`: `TYPE_GPS_DATA = 2`), plus EXIF timestamps. Sample
//! captures are the most useful thing this project can hand a stranger and the
//! easiest way to hand them the owner's home address at the same time.
//!
//! Deliberately prints coordinates rounded to ~1 km and groups captures into
//! distinct sites. That is enough to answer "how many places, are any of them
//! somewhere I live" without writing precise positions into a terminal log, a
//! scrollback buffer, or whatever an agent transcript ends up in.
//!
//! ```text
//! cargo run --release -p light --example privacy_scan -- .data-from-camera/raw/*.lri
//! ```

use lri_rs::LriFile;
use std::fs;

/// ~1 km at the equator, and less as you go north. Coarse enough not to be an
/// address, fine enough to tell two towns apart.
const GRID_DEG: f64 = 0.01;

fn main() {
	let paths: Vec<String> = std::env::args().skip(1).collect();
	if paths.is_empty() {
		eprintln!("usage: privacy_scan <file.lri> [more.lri ...]");
		std::process::exit(2);
	}

	let mut with_gps = Vec::new();
	let mut without = 0usize;
	let mut unreadable = 0usize;

	for p in &paths {
		let Ok(data) = fs::read(p) else {
			unreadable += 1;
			continue;
		};
		let Ok(lri) = LriFile::decode(&data) else {
			unreadable += 1;
			continue;
		};
		let short = p.rsplit('/').next().unwrap_or(p).to_string();
		match lri.fusion.gps {
			Some(fix) => with_gps.push((short, fix.latitude, fix.longitude, fix.altitude_m)),
			None => without += 1,
		}
	}

	println!("scanned {} file(s)", paths.len());
	println!("  with GPS fix:    {}", with_gps.len());
	println!("  without:         {without}");
	if unreadable > 0 {
		println!("  unreadable:      {unreadable}");
	}

	if with_gps.is_empty() {
		println!();
		println!("no GPS payload found — nothing to strip before publishing samples");
		return;
	}

	// Group to a coarse grid so the output says "how many places", not "where".
	let mut sites: std::collections::BTreeMap<(i64, i64), Vec<&String>> = Default::default();
	for (name, lat, lon, _) in &with_gps {
		let key = (
			(lat / GRID_DEG).round() as i64,
			(lon / GRID_DEG).round() as i64,
		);
		sites.entry(key).or_default().push(name);
	}

	println!();
	println!(
		"{} distinct site(s) at ~1 km resolution — coordinates rounded on purpose",
		sites.len()
	);
	for ((la, lo), files) in &sites {
		let lat = *la as f64 * GRID_DEG;
		let lon = *lo as f64 * GRID_DEG;
		println!(
			"   ~{lat:.2}, ~{lon:.2}   {} capture(s), e.g. {}",
			files.len(),
			files.first().map(|s| s.as_str()).unwrap_or("—")
		);
	}
	println!();
	println!("These are full-precision in the files themselves. Strip before publishing.");
}
