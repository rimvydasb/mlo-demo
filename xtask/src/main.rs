use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use voxel_core::BiomeCoord;

#[derive(Parser)]
#[command(name = "xtask", about = "Build and inspection automation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print ASCII top-down dump of the world map for a given seed.
    Dump {
        #[arg(long, default_value = "42")]
        seed: u64,
        #[arg(long, default_value = "0", help = "Row of biome to dump (0-5)")]
        row: u8,
        #[arg(long, default_value = "0", help = "Col of biome to dump (0-5)")]
        col: u8,
    },
    /// Run determinism check: generate seed twice and compare.
    Check {
        #[arg(long, default_value = "42")]
        seed: u64,
    },
    /// Render a screenshot via the native binary and save to disk.
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
        /// Disable the (non-deterministic) drifting clouds for byte-stable output.
        #[arg(long)]
        no_clouds: bool,
        /// Enable the optional MICROHEIGHT beautification rule (A/B flag).
        #[arg(long)]
        microheight: bool,
        /// Disable the fauna & flora decoration props (animated; use for
        /// byte-stable output).
        #[arg(long)]
        no_decor: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dump { seed, row, col } => {
            anyhow::ensure!(row < 6 && col < 6, "row and col must be in 0..5");
            let map = voxel_mapgen::generate(seed);
            println!("{}", voxel_mapgen::ascii_macro(&map));
            println!(
                "{}",
                voxel_mapgen::ascii_dump(&map, BiomeCoord::new(row, col))
            );
        }
        Command::Check { seed } => {
            let a = voxel_mapgen::generate(seed);
            let b = voxel_mapgen::generate(seed);
            let mut ok = true;
            for row in 0..6u8 {
                for col in 0..6u8 {
                    let coord = BiomeCoord::new(row, col);
                    if a.biome(coord) != b.biome(coord) {
                        eprintln!("MISMATCH at biome ({row},{col})");
                        ok = false;
                    }
                }
            }
            if ok {
                println!("OK — seed {seed} is deterministic");
            } else {
                anyhow::bail!("Determinism check failed for seed {seed}");
            }
        }
        Command::Screenshot {
            seed,
            out,
            row,
            col,
            no_clouds,
            microheight,
            no_decor,
        } => {
            let mut args = vec![
                "run".to_string(),
                "--quiet".into(),
                "-p".into(),
                "native".into(),
                "--".into(),
                "screenshot".into(),
                "--seed".into(),
                seed.to_string(),
                "--out".into(),
                out.to_str().context("non-UTF-8 output path")?.to_string(),
            ];
            if let Some(r) = row {
                args.extend(["--row".into(), r.to_string()]);
            }
            if let Some(c) = col {
                args.extend(["--col".into(), c.to_string()]);
            }
            if no_clouds {
                args.push("--no-clouds".into());
            }
            if microheight {
                args.push("--microheight".into());
            }
            if no_decor {
                args.push("--no-decor".into());
            }
            let status = std::process::Command::new("cargo").args(&args).status()?;
            anyhow::ensure!(status.success(), "screenshot command failed");
            println!("Screenshot written to {}", out.display());
        }
    }
    Ok(())
}
