use clap::{Parser, Subcommand};
use voxel_core::BiomeCoord;

#[derive(Parser)]
#[command(name = "voxel-mlm", about = "Voxel Mega-Lo-Mania — dev inspector")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Inspect {
        #[arg(long, default_value = "42")]
        seed: u64,
    },
    Screenshot {
        #[arg(long, default_value = "42")]
        seed: u64,
        #[arg(long, default_value = "screenshot.png")]
        out: std::path::PathBuf,
        #[arg(
            long,
            help = "Biome row to focus (0-5); defaults to the most scenic biome"
        )]
        row: Option<u8>,
        #[arg(
            long,
            help = "Biome col to focus (0-5); defaults to the most scenic biome"
        )]
        col: Option<u8>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { seed } => voxel_app::run_inspector(seed),
        Command::Screenshot {
            seed,
            out,
            row,
            col,
        } => {
            let biome = match (row, col) {
                (Some(r), Some(c)) if r < 6 && c < 6 => Some(BiomeCoord::new(r, c)),
                (None, None) => None,
                _ => {
                    eprintln!("--row and --col must both be given and be in 0..=5");
                    std::process::exit(1);
                }
            };
            voxel_app::run_screenshot(seed, out, biome)
        }
    }
}
