mod descriptions;
mod export;
mod http;
mod jobs;
mod photos;
mod regions;
mod sounds;
mod species;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf, time::Duration};

#[derive(Parser)]
#[command(about = "Builds the Quizzozoizo offline data set")]
struct Cli {
    /// Where downloaded responses are cached.
    #[arg(long, default_value = "data/cache")]
    cache: PathBuf,
    /// Where generated files are written.
    #[arg(long, default_value = "data/out")]
    out: PathBuf,
    /// Minimum delay between live API requests, in milliseconds.
    #[arg(long, default_value_t = 1100)]
    delay_ms: u64,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build the species list of every region (EN + FR names, taxonomy).
    Species {
        /// Ignore birds seen fewer times than this in a region.
        #[arg(long, default_value_t = 30)]
        min_count: i64,
        /// Each region contributes at most its N most observed birds.
        #[arg(long, default_value_t = 250)]
        top: usize,
    },
    /// Download photos of each bird.
    Photos {
        #[arg(long, default_value_t = 4)]
        per_species: usize,
    },
    /// Download feather photos (iNaturalist observations tagged "feather").
    Feathers {
        #[arg(long, default_value_t = 3)]
        per_species: usize,
    },
    /// Download xeno-canto sounds (needs XC_API_KEY in the environment).
    Sounds {
        #[arg(long, default_value_t = 2)]
        per_species: usize,
    },
    /// Fetch Wikipedia descriptions (EN + FR) and turn them into name-free riddles.
    Descriptions,
    /// Write catalog.json + media/ for the app to serve.
    Export {
        #[arg(long, default_value = "app/public")]
        to: PathBuf,
    },
}

fn load_species(out: &std::path::Path) -> Result<Vec<species::SpeciesRow>> {
    Ok(serde_json::from_slice(&fs::read(out.join("species.json"))?)?)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let http = http::Client::new(&cli.cache, Duration::from_millis(cli.delay_ms))?;
    fs::create_dir_all(&cli.out)?;
    match cli.cmd {
        Cmd::Species { min_count, top } => {
            let rows = species::fetch(&http, min_count, top)?;
            fs::write(cli.out.join("species.json"), serde_json::to_string_pretty(&rows)?)?;
            println!("{} species -> species.json", rows.len());
        }
        Cmd::Photos { per_species } => {
            let rows = photos::photos(&http, &cli.out, &load_species(&cli.out)?, per_species)?;
            println!("{} photos", rows.len());
        }
        Cmd::Feathers { per_species } => {
            let rows = photos::feathers(&http, &cli.out, &load_species(&cli.out)?, per_species)?;
            println!("{} feather photos", rows.len());
        }
        Cmd::Sounds { per_species } => {
            let key = std::env::var("XC_API_KEY").map_err(|_| anyhow::anyhow!("set XC_API_KEY (see .env)"))?;
            let rows = sounds::fetch(&http, &cli.out, &key, &load_species(&cli.out)?, per_species)?;
            println!("{} sounds", rows.len());
        }
        Cmd::Descriptions => {
            let d = descriptions::fetch(&http, &load_species(&cli.out)?)?;
            fs::write(cli.out.join("descriptions.json"), serde_json::to_string_pretty(&d)?)?;
            println!("{} descriptions -> descriptions.json", d.len());
        }
        Cmd::Export { to } => export::run(&cli.out, &to)?,
    }
    Ok(())
}
