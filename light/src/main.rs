use anyhow::Result;
use clap::{Parser, Subcommand};
use light::{extract, fuse, gather, validate_rt};

#[derive(Parser)]
#[command(
	name = "light",
	about = "Luminat — Light L16 fusion tooling",
	long_about = "Luminat — open 16→1 fusion for Light L16 (.lri decode, warp, blend). isamarin × BLMK",
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
		),
		Command::Extract { input, output, jobs } => extract::run(&input, &output, jobs),
	}
}