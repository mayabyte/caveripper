# Caveripper

Caveripper is an implementation of the Pikmin 2 cave generation algorithm intended for seed finding. Uses for this include, but are not limited to:
- Speedrunning - find the fastest single seed for an RTA run. (Or find the *slowest* seed for fun.)
- Challenge Mode Score Attack - floors with the maximum number of eggs have the highest possible scores.
- Tool Assisted Speedruns - find fastest overall seeds per floor to improve final TAS times.
- Finding "rare" seeds - some floors have possible but extremely rare phenomena that are simply fun to know about, e.g. Bloysterless SR7, Clackerless GK3, longest possible 'meme hallways', etc.
- Finding "interesting" seeds (high number of available bonus treasures, rare layout configurations, difficult score reads, etc.) for community races and practice.

## Project Status

This is a **work in progress** project. It cannot yet be used for seed finding and its cave generation implementation still has some minor inaccuracies.

While the original and main goal of this project is seed finding, I've found myself wanting to use Caveripper as a base for a whole host of other things too. As such, sub-goals related to seed finding specifically (new Judge algorithm) are on a 'whenever I feel like it' schedule; many of the recent additions have been made with other uses in mind.

## Building

### Extracting Pikmin 2 Game Assets
Game assets are not distributed in this repo, and as such you need to extract them from a game ISO you provide. This is made simple by the `extract_iso.sh` script provided. You will need [Wiimms ISO Tools](https://wit.wiimm.de/) and [Wiimms SZS Toolset](https://szs.wiimm.de/) (specifically `wit`, `wimgt`, and `wszst`) on your PATH, so make sure you download those first. Then just run the following:
```bash
./extract_iso.sh PATH_TO_PIKMIN_2_ISO.iso
```
This will extract the filesystem of the ISO, copy the necessary files into `assets/`, and decode the relevant SZS and BTI files into folders and PNG images, respectively. You should only need to do this once after cloning the repo, so make sure to have a Pikmin 2 ISO handy if you intend to build from source.

If this process fails for some reason and you want to clean up and start from scratch, just delete the `assets/` folder.

NOTE: This script is only set up to work with an NTSC-U (US) Pikmin 2 ISO currently. If you want to try this with another version, you may have to edit the script a bit to get it to play nice.

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
- `CaveGen/` is a submodule pointing to a fork of JHawk's Cavegen implementation I made for the sole purpose of testing the accuracy of my reference implementation. The modifications within are minor, but it's there if you're curious.

In case you're not familiar with Rust, `mod.rs` inside a folder is the main source file for code in that module and you should probably start there when reading.
