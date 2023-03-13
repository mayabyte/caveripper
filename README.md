# Caveripper

Caveripper is an implementation of the Pikmin 2 cave generation algorithm intended for seed finding. Uses for this include, but are not limited to:
- Speedrunning - find the fastest single seed for an RTA run. (Or find the *slowest* seed for fun.)
- Challenge Mode Score Attack - floors with the maximum number of eggs have the highest possible scores.
- Tool Assisted Speedruns - find fastest overall seeds per floor to improve final TAS times.
- Finding "rare" seeds - some floors have possible but extremely rare phenomena that are simply fun to know about, e.g. Bloysterless SR7, Clackerless GK3, longest possible 'meme hallways', etc.
- Finding "interesting" seeds (high number of available bonus treasures, rare layout configurations, difficult score reads, etc.) for community races and practice.

## How to Use
Make sure you extract Pikmin 2 game assets first!

The CLI has comprehensive help messages:
```bash
caveripper help
```

Here are some examples of common use-cases. All ouput images will be in the `output/` folder.
```bash
# Generate a layout image for a given sublevel and seed with quickglance rendering enabled.
caveripper generate scx3 0x1234abcd --quickglance

# Find a towerless seed.
caveripper search "scx7 MiniHoudai < 2"

# Find a seed that's both clackerless and candyless with no search timeout.
caveripper search "gk3 castanets = 0 & smc4 wealthy = 0" -t 0

# Compute the percentage of SR5 layouts with a Violet Candypop Bud.
caveripper stats "sr5 BlackPom = 1"
```

See [QUERY.md](QUERY.md) for a full explanation on Caveripper's query language.

Caveripper only recognizes *internal names* for game objects at present. If you want to see the internal names for things on a given floor, there's a handy text-only Caveinfo command that can be of assistance:
```bash
caveripper caveinfo fc3 --text
```

You can cross-reference which internal names correspond to which teki/rooms/treasures using this page on the Pikmin Technical Knowledge Base: https://pikmintkb.com/wiki/Pikmin_2_identifiers.

### Extracting Pikmin 2 Game Assets
Game assets are not distributed in this repo, and as such you need to extract them from a game ISO you provide. This is made simple by the `extract` command built into the Caveripper CLI:
```bash
caveripper extract path/to/pikmin2.iso
```

This will extract all the necessary files from the ISO into `~/.config/caveripper/assets` so Caveripper can find them from any location. You should only need to do this once, but it is absolutely necessary in order to use Caveripper. If you're worried about bloating your home directory, worry not: only ~12MB of assets are extracted per ISO.

If this process fails for some reason and you want to clean up and start from scratch, just delete the `assets/` folder in `~/.config/caveripper`, or simply re-extract your ISO and the extractor will clean up before extracting again.

## Project Status

This is a **work in progress** project. The cave generation implementation is not proven correct (but appears very close!) and seed finding capability is currently limited to basic query conditions.

While the original and main goal of this project is seed finding, I've found myself wanting to use Caveripper as a base for a whole host of other things too. As such, sub-goals related to seed finding specifically (new Judge algorithm) are on a 'whenever I feel like it' schedule; many of the recent additions have been made with other uses in mind.

## Installation
Pre-built binaries for Caveripper aren't provided currently. However, building from source is straightforward thanks to the install scripts (`install.sh` and `install.bat`) provided.

Make sure you have Rust installed using [Rustup](https://rustup.rs/):
```bash
rustup default nightly
rustup update
```
(Nightly Rust is required at the moment since Caveripper uses a few unstable language features.)

Then just run `install.sh` if you're on Linux/Macos/WSL or `install.bat` if you're on Windows. This will build the program, place the executable in your Cargo bin directory, and copy the Resources folder to `$HOME/.config/caveripper/resources` so it can be accessed from anywhere.

## Python Bindings
Caveripper comes with some very simple Python bindings to the core cave generation algorithm. You can use them by following these steps:
1. Follow the build steps above, but use the following build command instead: `cargo build --release -p bindings`
1. Find and rename the shared library file that was generated during the build. This will be in `target/release` next to the CLI binary.
    - On macOS, rename `libcaveripper.dylib` to `caveripper.so`.
    - On Windows, rename `caveripper.dll` or `libcaveripper.dll` to `caveripper.pyd`.
    - On Linux (including WSL), rename `libcaveripper.so` to `caveripper.so`.
1. Move the finished library file to your Python project's source directory along with the `assets/` and `resources/` folders.
1. Use Caveripper from Python!
```python
>>> import caveripper
>>> caveripper.generate(0x1234ABCD, "sh6")
{'name': 'SH6', 'seed': 305441741, 'ship': [510.0, 510.0], 'hole': [1955.0, 1615.0], 'geyser': None, 'map_units': [{'name': 'room_4x4b_4_conc', ...
```

### Python Binding Docs

```python
# Generates a sublevel layout and returns a serialized representation as
# a dictionary. If render=True, the image is saved as "<SUBLEVEL>_<SEED>.png".
generate(seed: int, sublevel: str, render: bool = False) -> dict

# Returns a serialized representation of the sublevel's full caveinfo
# struct heirarchy. See caveripper/src/caveinfo.rs for field and
# structure documentation.
caveinfo(sublevel: str) -> dict
```


## Guide to Reading the Code
If you're interested in the precise details of how cave generation works, I'd suggest reading the code directly rather than relying on explanations due to how particular the cave generation algorithm is. I attempt to keep this repository well-commented to facilitate this - please let me know and/or submit a PR if you feel that the comments can be improved!

General guide to the most important parts of the source tree:
- The `caveripper/` folder has the generation algorithm itself, plus all the other searching and analysis code that makes finding specific layouts possible.
- The `cli/` folder just contains the command line interface portion of the program - you probably don't need to read anything in here.
- `caveripper/src/caveinfo/` contains everything relating to loading, reading, and parsing the game's Caveinfo files.
- `caveripper/src/layout/generate.rs` contains the Cave Generation algorithm. This is currently a very straightforward port of the logic in JHawk's implementation, so reading it can be difficult in places.
- `caveripper/src/pikmin_math/` contains math and RNG functions that mirror those used in the real game.
- `caveripper/src/query/query.rs` is where the layout search conditions are defined. If you want to add a custom search condition, this file is probably the place to do it.
- `reference/` contains reference implementations in Java of certain important functions for comparison against my own implementations. These are largely copied from JHawk's implementation of Cavegen.
