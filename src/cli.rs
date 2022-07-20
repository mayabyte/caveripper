use cavegen::{sublevel::Sublevel, errors::SeedError, search::SearchCondition};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Commands,

    #[clap(short='v')]
    pub debug_logging: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[clap(arg_required_else_help=true)]
    Generate {
        #[clap(value_parser=|s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)})]
        sublevel: Sublevel,

        #[clap(value_parser=parse_seed)]
        seed: u32,
    },

    #[clap(arg_required_else_help=true)]
    Search {
        #[clap(value_parser=|s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)})]
        sublevel: Sublevel,

        #[clap(value_parser=|s: &str| {<SearchCondition as TryFrom<&str>>::try_from(s)})]
        condition: SearchCondition,
    },

    #[clap(arg_required_else_help=true)]
    Stats {
        #[clap(value_parser=|s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)})]
        sublevel: Sublevel,

        #[clap(value_parser=|s: &str| {<SearchCondition as TryFrom<&str>>::try_from(s)})]
        condition: SearchCondition,

        #[clap(default_value="10000")]
        num_to_search: usize,
    }
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