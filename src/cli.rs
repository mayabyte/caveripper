use cavegen::{sublevel::Sublevel, errors::SeedError};
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(value_parser=|s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)})]  // Need the closure here because lifetime inference is silly
    pub sublevel: Sublevel,

    #[clap(value_parser=parse_seed)]
    pub seed: u32,

    #[clap(short='v')]
    pub debug_logging: bool,
}

fn parse_seed(src: &str) -> Result<u32, SeedError> {
    let trimmed = src.strip_prefix("0x").unwrap_or(src);
    if trimmed.len() != 8 {
        Err(SeedError::InvalidLength)
    }
    else {
        u32::from_str_radix(trimmed, 16).map_err(|_| SeedError::InvalidHexDigits)
    }
}