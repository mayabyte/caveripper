# Dweevil - Caveripper for Web via WASM
https://mayabyte.github.io/caveripper/

Dweevil is a build of Caveripper compiled to a WASM blob that can be run entirely client-side in web browsers. This is intended for demo purposes as well as embedding in other sites such as [Pikipedia](https://www.pikminwiki.com/). 

All required *vanilla Pikmin 2* assets are bundled into the WASM blob, so Dweevil does not need to make any network requests or store any cookies to operate. This does however result in an approx. 7MB bundle, so it's not an ideal experience at the moment. Future versions of Dweevil may have configuration options for fetching images (by far the largest part of the bundled data) from servers, such as Pikipedia itself. Dweevil does not support any ROM hacks since this would balloon the bundle size.

## Development
### Building and Running
1. You need an extracted Pikmin 2 assets folder for the build process. Run `caveripper extract GPVE01.iso -o .` in the root of the repo.
1. In `dweevil/www`, run `npm run start`. You only have to do this once - changes will hot-reload afterwards.
1. In another terminal window, run `wasm-pack build` in `dweevil` every time you change Rust code.
1. Connect at `localhost:8080`

### Deploying (mostly just so I can remember what to do lol)
1. `npm run build`
1. `npm run deploy`