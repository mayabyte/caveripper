# Caveripper

Caveripper is an implementation of the Pikmin 2 cave generation algorithm intended for seed finding. Uses for this include, but are not limited to:
- Speedrunning - find the fastest single seed for an RTA run. (Or find the *slowest* seed for fun.)
- Challenge Mode Score Attack - floors with the maximum number of eggs have the highest possible scores.
- Tool Assisted Speedruns - find fastest overall seeds per floor to improve final TAS times.
- Finding "rare" seeds - some floors have possible but extremely rare phenomena that are simply fun to know about, e.g. Bloysterless SR7, Clackerless GK3, longest possible 'meme hallways', etc.
- Finding "interesting" seeds (high number of available bonus treasures, rare layout configurations, difficult score reads, etc.) for community races and practice.

## How to Use
The CLI has comprehensive help messages:
```bash
caveripper help
```

Here are some examples of common use-cases. All ouput images will be in the `output/` folder.
```bash
# Generate a layout image for a given sublevel and seed with quickglance rendering enabled.
caveripper generate scx3 0x1234abcd --quickglance

# Find a towerless seed and render an image of it.
caveripper search scx7 "MiniHoudai < 2" -r

# Compute the percentage of SR5 layouts with a Violet Candypop Bud.
caveripper stats sr5 "BlackPom = 1"
```

Caveripper only recognizes *internal names* for game objects at present. If you want to see the internal names for things on a given floor, there's a handy text-only Caveinfo command that can be of assistance:
```bash
caveripper caveinfo fc3 --text
```

You can cross-reference which internal names correspond to which teki/rooms/treasures using this page on the Pikmin Technical Knowledge Base: https://pikmintkb.com/wiki/Pikmin_2_identifiers.

## Project Status

This is a **work in progress** project. The cave generation implementation still has some minor inaccuracies and seed finding capability is currently limited to basic query conditions.

While the original and main goal of this project is seed finding, I've found myself wanting to use Caveripper as a base for a whole host of other things too. As such, sub-goals related to seed finding specifically (new Judge algorithm) are on a 'whenever I feel like it' schedule; many of the recent additions have been made with other uses in mind.

## Building from source

### Extracting Pikmin 2 Game Assets
Game assets are not distributed in this repo, and as such you need to extract them from a game ISO you provide. This is made simple by the `extract_iso.sh` script provided. You will need [Wiimms ISO Tools](https://wit.wiimm.de/) and [Wiimms SZS Toolset](https://szs.wiimm.de/) (specifically `wit` and `wszst`) on your PATH, and you need Python 3 installed.

The `extract_bti.py` script is used for decoding BTI images rather than `wimgt` due to some edge-cases that `wimgt` can't handle. All code from `extract_bti.py` is copied directly from [GameCube File Tools by LagoLunatic](https://github.com/LagoLunatic/GCFT) (which is a great tool by the way!) and reformatted/trimmed down for use in scripts. Its only dependency is Pillow, which you can install like this:
```bash
python3 -m pip install Pillow==9.0.1
```

Then just run the following:
```bash
./extract_iso.sh PATH_TO_PIKMIN_2_ISO.iso
```
This will extract the filesystem of the ISO, copy the necessary files into `assets/`, and decode the relevant SZS and BTI files into folders and PNG images, respectively. You should only need to do this once after cloning the repo, so make sure to have a Pikmin 2 ISO handy if you intend to build from source.

If this process fails for some reason and you want to clean up and start from scratch, just delete the `assets/` folder.

NOTE: This repo is only set up to work with an NTSC-U (US) Pikmin 2 ISO currently. If you want to try this with another version, you may have to edit the script and/or the code a bit to get it to play nice.

### Building
Caveripper is a Rust project, and as such building should be very straightforward. Make sure you have Rust installed using [Rustup](https://rustup.rs/): 
```bash
rustup default nightly
rustup update
```

(Nightly Rust is required at the moment since Caveripper uses a few unstable language features.)

Then you can build the Caveripper executable:
```bash
cargo build --release
```

The finished executable will be `target/release/caveripper` (Mac/Linux) or `target\release\caveripper.exe` (Windows). The `assets/` and `resources/` directories must be in the current working directory when run.

### Running Tests and Benchmarks
The test and benchmark suite is currently sparse, not especially comprehensive, and likely very fragile, and as such I'd recommend you don't bother with this unless you're trying to develop Caveripper yourself.

Tests can be run as follows:
```bash
cargo test -- --nocapture
```

Benchmarks can be run like this:
```bash
cargo install cargo-criterion  # one-time installation of the benchmark harness
cargo criterion  # run benchmarks
```

## Guide to Reading the Code
If you're interested in the precise details of how cave generation works, I'd suggest reading the code directly rather than relying on explanations due to how particular the cave generation algorithm is. I attempt to keep this repository well-commented to facilitate this - please let me know and/or submit a PR if you feel that the comments can be improved!

General guide to the most important parts of the source tree:
- The `caveripper/` folder has the generation algorithm itself, plus all the other searching and analysis code that makes finding specific layouts possible.
- The `cli/` folder just contains the command line interface portion of the program - you probably don't need to read anything in here.
- `caveripper/src/caveinfo/` contains everything relating to loading, reading, and parsing the game's Caveinfo files.
- `caveripper/src/layout/generate.rs` contains the Cave Generation algorithm. This is currently a very straightforward port of the logic in JHawk's implementation, so reading it can be difficult in places.
- `caveripper/src/pikmin_math/` contains math and RNG functions that mirror those used in the real game.
- `caveripper/src/search.rs` is where the layout search conditions are defined. If you want to add a custom search condition, this file is probably the place to do it.
- `reference/` contains reference implementations in Java of certain important functions for comparison against my own implementations. These are largely copied from JHawk's implementation of Cavegen.
- `CaveGen/` is a submodule pointing to a fork of JHawk's Cavegen implementation I made for the sole purpose of testing the accuracy of my reference implementation. The modifications within are minor, but it's there if you're curious.
