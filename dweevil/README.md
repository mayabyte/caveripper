# Dweevil - Caveripper for Web via WASM

## Building and Running
1. You need an extracted Pikmin 2 assets folder for the build process. Run `caveripper extract GPVE01.iso -o .` in the root of the repo.
1. In `dweevil/www`, run `npm run start`. You only have to do this once - changes will hot-reload afterwards.
1. In another terminal window, run `wasm-pack build` in `dweevil` every time you change Rust code.
1. Connect at `localhost:8080`

## Deploying (mostly just so I can remember what to do lol)
1. `npm run build`
1. `npm run deploy`