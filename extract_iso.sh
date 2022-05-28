#!/bin/bash
set -e

wit EXTRACT "$1" ./p2filesystem
mkdir -p ./assets
cp -r ./p2filesystem/P-GPVE/files/user/Mukki/mapunits/* ./assets
find ./assets -iname "*.szs" -execdir wszst EXTRACT {} \;
find ./assets -iname "*.bti" -execdir wimgt DECODE {} \;
rm -rf ./p2filesystem
