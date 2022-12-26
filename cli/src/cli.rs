use std::path::PathBuf;

use caveripper::{parse_seed, sublevel::Sublevel, query::{Query, QUERY_HELP}, render::{LayoutRenderOptions, CaveinfoRenderOptions}};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(name="caveripper", author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Commands,

    #[clap(
        global = true,
        default_value_t = 0,
        short = 'v',
        help = VERBOSE_HELP,
    )]
    pub verbosity: u8,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Generate a sublevel layout and render an image of it.
    #[clap(
        arg_required_else_help = true,
    )]
    Generate {
        #[clap(
            value_parser = |s: &str| {<Sublevel as TryFrom<&str>>::try_from(s).map_err(|e| format!("{e:#?}"))},
            help = SUBLEVEL_HELP,
        )]
        sublevel: Sublevel,

        #[clap(
            value_parser = |s: &str| parse_seed(s).map_err(|e| format!("{e:#?}")),
            help = SEED_HELP,
        )]
        seed: u32,

        #[clap(flatten)]
        render_options: LayoutRenderOptions,
    },

    /// Display a particular sublevel's CaveInfo.
    #[clap(
        arg_required_else_help = true,
    )]
    Caveinfo {
        #[clap(
            value_parser = |s: &str| {<Sublevel as TryFrom<&str>>::try_from(s).map_err(|e| format!("{e:#?}"))},
            help = SUBLEVEL_HELP,
        )]
        sublevel: Sublevel,

        #[clap(
            short = 't',
            long = "text",
            help = "Only show text instead of rendering an image"
        )]
        text: bool,

        #[clap(flatten)]
        render_options: CaveinfoRenderOptions,
    },

    /// Search for a seed matching a specified condition.
    #[clap(
        arg_required_else_help = true,
    )]
    Search {
        #[clap(
            value_parser = |s: &str| {<Query as TryFrom<&str>>::try_from(s).map_err(|e| format!("{e:#?}"))},
            help = SEARCH_COND_HELP,
            long_help = QUERY_HELP,
        )]
        query: Query,

        #[clap(
            default_value_t = 10,
            short = 't',
            long = "timeout",
            help = "The maximum time to search for a layout, in seconds. If set to 0, search indefinitely"
        )]
        timeout_s: u64,

        #[clap(
            default_value_t = 1,
            short = 'n',
            long = "num",
            help = "Number of seeds to attempt to find."
        )]
        num: usize,

        #[clap(
            value_parser = |s: &str| parse_seed(s).map_err(|e| format!("{e:#?}")),
            short = 's',
            long = "start",
            help = "Search seeds sequentially starting from the given seed."
        )]
        start_from: Option<u32>,
    },

    /// Calculate statistics on what proportion of seeds match a given condition.
    #[clap(
        arg_required_else_help = true,
    )]
    Stats {
        #[clap(
            value_parser = |s: &str| {<Query as TryFrom<&str>>::try_from(s).map_err(|e| format!("{e:#?}"))},
            help = SEARCH_COND_HELP,
            long_help = QUERY_HELP,
        )]
        query: Query,

        #[clap(
            default_value = "100000",
            short = 'n',
            long = "num-to-search",
            help = "Number of seeds to check. Larger sample sizes will produce more reliable results."
        )]
        num_to_search: usize,
    },

    /// Accepts input seeds from a file or stdin, and only prints those that
    /// match the query condition.
    #[clap(
        arg_required_else_help = true,
    )]
    Filter {
        #[clap(
            value_parser = |s: &str| {<Query as TryFrom<&str>>::try_from(s).map_err(|e| format!("{e:#?}"))},
            help = SEARCH_COND_HELP,
            long_help = QUERY_HELP,
        )]
        query: Query,

        #[clap(
            long_help = SEED_FILE_HELP,
        )]
        file: Option<String>,
    },

    /// Extracts a game ISO interactively.
    #[clap(
        arg_required_else_help = true,
    )]
    Extract {
        #[clap(
            help = "The ISO file to extract."
        )]
        iso_path: PathBuf,

        #[clap(
            help = "The name for this ISO. Will attempt to auto-detect if not provided.",
        )]
        game_name: Option<String>,
    }
}

const SUBLEVEL_HELP: &str = "The sublevel in question. Examples: \"SCx6\", \"SmC-3\", \"bk4\"";
const SEARCH_COND_HELP: &str = "A condition to search for in the sublevel.";
const SEED_HELP: &str =
r##"The seed to check. Must be an 8-digit hexadecimal number, optionally prefixed
with "0x". Not case sensitive.
Examples: "0x1234ABCD", "baba2233".
"##;
const VERBOSE_HELP: &str = "Enable debug logging. Repeat up to 3 times to increase verbosity.";
const SEED_FILE_HELP: &str =
r##"The file to read seeds from. Should contain one seed on each line with no extra
punctuation. If not specified, reads from STDIN.
"##;
