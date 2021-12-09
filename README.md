# Caveripper

Caveripper is an implementation of the Pikmin 2 cave generation algorithm to be used for seed finding.

## Building

### Extracting Pikmin 2 Game Assets
Game assets are not distributed in this repo, and as such you need to extract them from a game ISO you provide. This is made simple by the `extract_iso.sh` script provided. Just run the following:
```bash
./extract_iso.sh PATH_TO_PIKMIN_2_ISO.iso
```
This will extract the filesystem of the ISO, copy the necessary files into `assets/`, and decode the relevant SZS and BTI files into folders and PNG images, respectively. You should only need to do this once after cloning the repo, so make sure to have a Pikmin 2 ISO handy if you intend to build from source.

For those who are curious, `tools/` contains specific executables from Wiimms ISO Tools (https://wit.wiimm.de/) and Wiimms SZS Toolset (https://szs.wiimm.de/) required to extract the game data. You shouldn't need to download any dependencies for this script.

### Building and Running Tests
Caveripper is a Rust project, and as such building is very simple. Make sure you have Rust installed (I recommend using Rustup: https://rustup.rs/), then use the following commands:
```bash
cargo test
cargo criterion  # run benchmarks
cargo build --release
```
The finished executable will be `target/release/caveripper` (or `target\release\caveripper.exe` on Windows) and should be completely stand-alone.

## Guide to Reading the Code
If you're interested in the nitty-gritty details of how the program works, I'd suggest reading the code directly rather than relying on explanations due to how particular the cave generation algorithm is. I attempt to keep this repository well-commented to facilitate this - please let me know and/or submit a PR if you feel that the comments can be improved!

General guide to the most important parts of the source tree:
- `src/caveinfo/` contains everything relating to loading, reading, and parsing the game's Caveinfo files.
- `src/layout/` contains the Cave Generation algorithm.
- `src/pikmin_math/` contains math and RNG functions that mirror those used in the real game.
- `reference/` contains reference implementations in Java of certain important functions for comparison against my own implementations. These are largely copied from JHawk's implementation of Cavegen.
- `assets/caveinfo/` contains the primary game data for how layouts are generated. The file format is described here: https://pikmintkb.com/wiki/Cave_generation_parameters

In case you're not familiar with Rust, `mod.rs` inside a folder is the main source file for code in that module and you should probably start there when reading.
