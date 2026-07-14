use anyhow::Result;
use clap::{Parser, Subcommand};
use light::{extract, gather, validate_rt};

#[derive(Parser)]
#[command(
	name = "light",
	about = "Light L16 LRI tooling",
	long_about = "Light L16 LRI tooling — isamarin × BLMK",
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
		Command::Extract { input, output, jobs } => extract::run(&input, &output, jobs),
	}
}