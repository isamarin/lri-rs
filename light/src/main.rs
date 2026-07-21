use anyhow::Result;
use clap::{Parser, Subcommand};
use light::{extract, fuse, gather, grid, validate_rt};

#[derive(Parser)]
#[command(
	name = "light",
	about = "Luminat — illuminate the 16→1 ritual",
	long_about = "\
Luminat — a not-so-secret society for Light L16.\n\n\
Sixteen modules witness; one image emerges. We decode .lri, undistort, warp, and blend — \
the fusion rite Lumen kept behind closed doors.\n\n\
All seeing is computational. isamarin × BLMK",
	version
)]
struct Cli {
	#[command(subcommand)]
	command: Command,
}

#[derive(Subcommand)]
enum Command {
	/// Print metadata for all .lri files in a directory
	Gather {
		/// Directory containing .lri / .jpg / .lris files
		path: camino::Utf8PathBuf,
	},
	/// Validate R/t by warping module previews and comparing to Lumen fused JPG
	Validate {
		/// Input .lri file
		#[arg(long)]
		lri: camino::Utf8PathBuf,
		/// Lumen fused output .jpg
		#[arg(long)]
		lumen: camino::Utf8PathBuf,
		/// Output directory for overlays and metrics
		#[arg(short, long)]
		output: camino::Utf8PathBuf,
		/// Longest preview side in pixels (default 1024)
		#[arg(long, default_value_t = 1024)]
		max_side: u32,
	},
	/// Luminat fuse: undistort, depth warp, blend; optional full-res TIFF/DNG + crop
	Fuse {
		#[arg(long)]
		lri: camino::Utf8PathBuf,
		#[arg(short, long)]
		output: camino::Utf8PathBuf,
		#[arg(long)]
		lumen: Option<camino::Utf8PathBuf>,
		/// Preview longest side (ignored when --full-res)
		#[arg(long, default_value_t = 1024)]
		max_side: u32,
		/// Fuse at Lumen canvas (10432×7824) and export 16-bit TIFF/DNG with crop
		#[arg(long)]
		full_res: bool,
		#[arg(long, default_value_t = true)]
		export_tiff: bool,
		#[arg(long, default_value_t = true)]
		export_dng: bool,
		#[arg(long, default_value_t = 1500.0)]
		depth_min_mm: f64,
		#[arg(long, default_value_t = 8000.0)]
		depth_max_mm: f64,
		#[arg(long, default_value_t = 25)]
		depth_steps: usize,
	},
	/// Contact sheet: per-module previews plus a labelled 16-cell grid
	///
	/// Every module gets a cell whether or not it fired, so a defect that hits a
	/// subset (see OPEN-QUESTIONS.md §1) is visible instead of averaged away.
	Grid {
		/// Input .lri file
		input: camino::Utf8PathBuf,
		/// Output directory (grid.png plus one png per module)
		output: camino::Utf8PathBuf,
	},
	/// Extract per-camera DNGs from one LRI file
	Extract {
		/// Input .lri file
		input: camino::Utf8PathBuf,
		/// Output directory
		output: camino::Utf8PathBuf,
		/// Parallel export jobs (default: logical CPU count)
		#[arg(short, long)]
		jobs: Option<usize>,
	},
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	match cli.command {
		Command::Gather { path } => gather::run(&path),
		Command::Validate {
			lri,
			lumen,
			output,
			max_side,
		} => validate_rt::run(&lri, &lumen, &output, max_side),
		Command::Fuse {
			lri,
			output,
			lumen,
			max_side,
			full_res,
			export_tiff,
			export_dng,
			depth_min_mm,
			depth_max_mm,
			depth_steps,
		} => fuse::run(
			&lri,
			&output,
			lumen.as_deref(),
			max_side,
			full_res,
			export_tiff,
			export_dng,
			depth_min_mm,
			depth_max_mm,
			depth_steps,
		)
		.map(|_| ()),
		Command::Grid { input, output } => grid::run(&input, &output),
		Command::Extract { input, output, jobs } => extract::run(&input, &output, jobs),
	}
}