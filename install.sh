#!/bin/bash

set -e

echo "Installing Caveripper"
cargo install --path ./cli

echo "Copying resources folder into $HOME/.config/caveripper/"
mkdir -p ~/.config/caveripper
cp -r resources ~/.config/caveripper

echo "Done. Make sure your Cargo bin directory is on PATH (~/.cargo/bin on Linux/Macos)."
