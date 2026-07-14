use anyhow::Result;
use clap::{Parser, Subcommand};

mod extract;
mod gather;
mod render;

#[derive(Parser)]
#[command(name = "light", about = "Light L16 LRI tooling", version)]
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
	/// Extract per-camera PNGs from one LRI file
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
		Command::Extract { input, output, jobs } => extract::run(&input, &output, jobs),
	}
}