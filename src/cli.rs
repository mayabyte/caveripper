use cavegen::{sublevel::Sublevel, errors::SeedError, search::Query, layout::render::RenderOptions};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Commands,

    #[clap(
        global = true,
        short = 'v',
        parse(from_occurrences),
        takes_value = false,
        multiple_occurrences = true,
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
            value_parser = |s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)},
            help = SUBLEVEL_HELP,
        )]
        sublevel: Sublevel,
    
        #[clap(
            value_parser = parse_seed,
            help = SEED_HELP,
        )]
        seed: u32,

        #[clap(flatten)]
        render_options: RenderOptions,
    },

    #[clap(
        arg_required_else_help = true,
    )]
    Caveinfo {
        #[clap(
            value_parser = |s: &str| {<Sublevel as TryFrom<&str>>::try_from(s)},
            help = SUBLEVEL_HELP,
        )]
        sublevel: Sublevel,

        #[clap(
            short = 't',
            long = "text",
            help = "Only show text instead of rendering an image"
        )]
        text: bool,
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
            value_parser = |s: &str| {<Query as TryFrom<&str>>::try_from(s)},
            help = SEARCH_COND_HELP,
            long_help = SEARCH_COND_LONG_HELP,
        )]
        query: Query,

        #[clap(
            default_value = "10",
            short = 't',
            long = "timeout",
            help = "The maximum time to search for a layout, in seconds. If set to 0, search indefinitely"
        )]
        timeout: u64,

        #[clap(
            short = 'r',
            long = "render",
            help = "Render the found layout immediately"
        )]
        render: bool,

        #[clap(flatten)]
        render_options: RenderOptions,
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
            value_parser = |s: &str| {<Query as TryFrom<&str>>::try_from(s)},
            help = SEARCH_COND_HELP,
            long_help = SEARCH_COND_LONG_HELP,
        )]
        query: Query,

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
A string with one or more query conditions, joined by '&'. Caveripper will attempt 
to find a layout matching all conditions.

Currently available query conditions:
- "INTERNAL_NAME </=/> NUM". Checks the number of the named entity present in
  each layout. This can include Teki, Treasures, or gates (use the name "gate").
  Example: "BlackPom > 0" to check for layouts that have at least one Violet 
  Candypop Bud.
- "ROOM </=/> NUM". Check the number of the given unit type present. ROOM can be
  a specific room's name, or "room", "alcove", or "hallway" to check all rooms of
  a certain type. Example: "alcove < 2" to check for low-alcove layouts.
- "ENTITY in ROOM". Check whether there's at least one of the named entity in one
  of the specified rooms/room types. Example: "hole in room" to check if the exit
  hole spawned in a room rather than an alcove.
- "ENTITY1 with ENTITY2". Check whether the two named entities are in the same room
  as each other.
"##;
const SEED_HELP: &'static str = r##"
The seed to check. Must be an 8-digit hexadecimal number, optionally prefixed with "0x". Not case sensitive.
Examples: "0x1234ABCD", "baba2233".
"##;
const VERBOSE_HELP: &'static str = "Enable debug logging.";
