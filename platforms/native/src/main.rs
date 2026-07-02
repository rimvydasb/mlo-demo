use clap::{Args, Parser, Subcommand};
use voxel_core::BiomeCoord;

#[derive(Parser)]
#[command(name = "voxel-mlm", about = "Voxel Mega-Lo-Mania — dev inspector")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone, Copy)]
struct Presentation {
    /// Disable the drifting clouds (they are intentionally non-deterministic;
    /// use this for byte-stable screenshots).
    #[arg(long)]
    no_clouds: bool,
    /// Enable the optional MICROHEIGHT beautification rule (A/B flag).
    #[arg(long)]
    microheight: bool,
    /// Disable the fauna & flora decoration props (their idle animations are
    /// time-based; use this for byte-stable screenshots).
    #[arg(long)]
    no_decor: bool,
}

impl From<Presentation> for voxel_app::AppOptions {
    fn from(p: Presentation) -> Self {
        Self {
            clouds: !p.no_clouds,
            microheight: p.microheight,
            decor: !p.no_decor,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    Inspect {
        #[arg(long, default_value = "42")]
        seed: u64,
        #[command(flatten)]
        presentation: Presentation,
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
        #[command(flatten)]
        presentation: Presentation,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { seed, presentation } => {
            voxel_app::run_inspector(seed, presentation.into())
        }
        Command::Screenshot {
            seed,
            out,
            row,
            col,
            presentation,
        } => {
            let biome = match (row, col) {
                (Some(r), Some(c)) if r < 6 && c < 6 => Some(BiomeCoord::new(r, c)),
                (None, None) => None,
                _ => {
                    eprintln!("--row and --col must both be given and be in 0..=5");
                    std::process::exit(1);
                }
            };
            voxel_app::run_screenshot(seed, out, biome, presentation.into())
        }
    }
}
