#!/bin/bash
set -e

wit EXTRACT "$1" ./p2filesystem
mkdir -p ./assets
mkdir -p ./assets/resulttex
mkdir -p ./assets/enemytex
cp -r ./p2filesystem/P-GPVE/files/user/Mukki/mapunits/* ./assets
cp -r ./p2filesystem/P-GPVE/files/user/Matoba/resulttex/* ./assets/resulttex
cp ./p2filesystem/P-GPVE/files/user/Yamashita/enemytex/arc.szs ./assets/enemytex
find ./assets -iname "*.szs" -execdir wszst EXTRACT {} \;
find ./assets -iname "*.bti" -execdir wimgt DECODE {} \;
rm -rf ./p2filesystem
