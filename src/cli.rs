use cavegen::{sublevel::Sublevel, errors::SeedError, search::SearchCondition};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Commands,

    #[clap(
        global = true,
        short = 'v',
        help = VERBOSE_HELP,
    )]
    pub debug_logging: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Generate a sublevel layout and render an image of it.
    #[clap(
        arg_required_else_help = true,
    )]
    Generate {
        #[clap(
            value_parser = |s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)},
            help = SUBLEVEL_HELP,
        )]
        sublevel: Sublevel,
    
        #[clap(
            value_parser = parse_seed,
            help = SEED_HELP,
        )]
        seed: u32,
    },

    /// Search for a seed matching a specified condition.
    #[clap(
        arg_required_else_help = true,
    )]
    Search {
        #[clap(
            value_parser = |s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)},
            help = SUBLEVEL_HELP,
        )]
        sublevel: Sublevel,

        #[clap(
            value_parser = |s: &str| {<SearchCondition as TryFrom<&str>>::try_from(s)},
            help = SEARCH_COND_HELP,
            long_help = SEARCH_COND_LONG_HELP,
        )]
        condition: SearchCondition,

        #[clap(
            default_value = "10",
            short = 't',
            long = "timeout",
            help = "The maximum time to search for a layout, in seconds."
        )]
        timeout: u64,
    },

    /// Calculate statistics on what proportion of seeds match a given condition.
    #[clap(
        arg_required_else_help = true,
    )]
    Stats {
        #[clap(
            value_parser = |s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)},
            help = SUBLEVEL_HELP,
        )]
        sublevel: Sublevel,

        #[clap(
            value_parser = |s: &str| {<SearchCondition as TryFrom<&str>>::try_from(s)},
            help = SEARCH_COND_HELP,
            long_help = SEARCH_COND_LONG_HELP,
        )]
        condition: SearchCondition,

        #[clap(
            default_value = "10000",
            short = 'n',
            long = "num-to-search",
        )]
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

const SUBLEVEL_HELP: &'static str = "The sublevel in question. Examples: \"SCx6\", \"SmC-3\", \"bk4\"";
const SEARCH_COND_HELP: &'static str = "A condition to search for in the sublevel.";
const SEARCH_COND_LONG_HELP: &'static str = r##"
A string with a query condition to search for.

Currently available query conditions:
- "count INTERNAL_NAME </=/> NUM". Checks the number of the named entity present in
  each layout. This can include Teki, Treasures, or gates (use the name "gate").
"##;
const SEED_HELP: &'static str = r##"
The seed to check. Must be an 8-digit hexadecimal number, optionally prefixed with "0x". Not case sensitive.
Examples: "0x1234ABCD", "baba2233".
"##;
const VERBOSE_HELP: &'static str = "Enable debug logging.";
