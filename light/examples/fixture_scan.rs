//! Which captures fire which modules — the fixture question, asked of all 61.
//!
//! Several open questions are blocked less on understanding than on finding a
//! capture where the right modules fire together. Searching what we already have
//! is cheaper than shooting something new, and it costs one pass over the
//! archive. Run it before concluding that a fixture does not exist.
//!
//! ```text
//! cargo run --release -p light --example fixture_scan -- .data-from-camera/raw/*.lri
//! ```

use lri_rs::{CameraId, LriFile};
use std::collections::HashSet;
use std::fs;

fn main() {
	let paths: Vec<String> = std::env::args().skip(1).collect();
	let mut both = Vec::new();
	let mut rows = Vec::new();

	for p in &paths {
		let Ok(data) = fs::read(p) else { continue };
		let Ok(lri) = LriFile::decode(&data) else { continue };
		let fired: HashSet<CameraId> = lri.images.iter().map(|i| i.camera).collect();
		let name = |c: &CameraId| format!("{c:?}");
		let has = |s: &str| fired.iter().any(|c| name(c) == s);

		let a_row = ["A1", "A2", "A3", "A4", "A5"]
			.iter()
			.filter(|s| has(s))
			.count();
		let glued_c = ["C5", "C6"].iter().filter(|s| has(s)).count();
		let short = p.rsplit('/').next().unwrap_or(p).to_string();
		let focal = lri.focal_length.unwrap_or(0);
		let refc = lri
			.image_reference_camera
			.map(|c| name(&c))
			.unwrap_or_else(|| "—".into());

		if a_row > 0 && glued_c > 0 {
			both.push((short.clone(), focal, refc.clone(), a_row, glued_c));
		}
		rows.push((short, focal, refc, a_row, glued_c, fired.len()));
	}

	println!(
		"{:<16} {:>6} {:>5} {:>6} {:>6} {:>6}",
		"capture", "focal", "ref", "A row", "C5/C6", "fired"
	);
	for (n, f, r, a, g, total) in &rows {
		println!("{n:<16} {f:>6} {r:>5} {a:>6} {g:>6} {total:>6}");
	}
	println!();
	if both.is_empty() {
		println!("no capture fires the A row and C5/C6 together — the fixture has to be shot");
	} else {
		println!(
			"{} capture(s) fire BOTH the A row and glued C — no new capture needed:",
			both.len()
		);
		for (n, f, r, a, g) in &both {
			println!("   {n}  focal {f}  ref {r}  ({a} A modules, {g} glued C)");
		}
	}
}
