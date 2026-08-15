# Zerlak Frontier

A galaxy-map strategy layer and first-person space-combat sector, built in Rust
with [Bevy](https://bevyengine.org/). Pick a sector on the galaxy map, warp in,
and fight off the Zerlak fleet before your fuel or your hull gives out.

**Play it in the browser:** <https://westfield.dev/zerlak-frontier/>

## About / attribution

Zerlak Frontier is an unofficial, non-commercial fan tribute inspired by **Solaris**,
the 1986 Atari 2600 game by Douglas Neubauer, published by Atari Corporation.
It borrows the broad two-layer shape of that game (a galaxy map you navigate
sector by sector, and a first-person combat view when you engage the enemy)
and its "Zerlak" antagonists as an homage — it is an original implementation,
not a port, clone, or asset reuse, and shares no code or art with the
original.

This project is **not affiliated with, endorsed by, or sponsored by Atari**.
"Solaris," "Atari," and all related names, characters, and trademarks are the
property of their respective owners. Zerlak Frontier is offered free of charge for
personal, educational, and non-commercial use only — see [License](#license).

## How to play

### Galaxy map

- **Arrows / mouse / tap:** pick a sector
- **Enter / Space / click / tap:** warp into the selected sector
- Red sectors are Zerlak-held — warping in starts a combat sector. Blue
  sectors are friendly outposts that refuel you. Decide fast: if you sit on
  a non-Zerlak choice too long, the countdown in the corner runs out and a
  Zerlak sector gets picked (and warped into) for you.
- **Esc:** quit to the title screen

### Flight (combat)

- **Arrows / mouse / touch drag:** aim the crosshair
- **Space / click / tap:** fire
- Watch for enemies charging a laser lock on your crosshair — move away
  before it fires or take damage. Destroy every fighter in the sector to
  clear it.
- **Esc:** retreat to the galaxy map

Running out of fuel or hull health ends the run. Clearing every Zerlak sector
regenerates a bigger, tougher galaxy for the next level — there's no fixed
ending, just an escalating campaign.

## Running it natively

Requires a recent stable [Rust toolchain](https://rustup.rs/).

```sh
cargo run --release
```

## Building for the web

The `web/` directory is a small [Vite](https://vitejs.dev/) + React
front end that hosts the game's WebAssembly build in a letterboxed canvas,
usable on desktop, tablet, and phone browsers.

```sh
# 1. Build the Rust game for wasm32
rustup target add wasm32-unknown-unknown
# The version must match whatever wasm-bindgen bevy/rand resolve to in
# Cargo.lock (currently 0.2.127) or wasm-bindgen will refuse to run.
cargo install --locked wasm-bindgen-cli --version 0.2.127
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/src/wasm --out-name solaris \
  target/wasm32-unknown-unknown/release/solaris.wasm

# 2. Build the front end
mkdir -p web/public/assets && cp -r assets/. web/public/assets/
cd web
npm install
npx wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
  -o src/wasm/solaris_bg.wasm src/wasm/solaris_bg.wasm
npm run build   # outputs web/dist
```

## Deployment

Pushing to `main`/`master` runs `.github/workflows/deploy.yml`, which builds
the wasm binary and the React front end, then publishes `web/dist` to GitHub
Pages automatically (Pages is already configured under Settings → Pages →
**Source: GitHub Actions**). The URL above goes live after the first
successful workflow run.

## License

Source code is licensed under the **[PolyForm Noncommercial License
1.0.0](LICENSE)** — free to use, modify, and share for any non-commercial
purpose, with attribution. See the [`LICENSE`](LICENSE) file for the full
text. This keeps the project's terms consistent with the non-commercial,
fan-tribute spirit described in [Attribution](#about--attribution) above.

The title screen uses [Audiowide](https://fonts.google.com/specimen/Audiowide)
by Brian J. Bonislawsky, licensed under the [SIL Open Font License
1.1](assets/fonts/OFL.txt).

---

Written w/ [Claude Code](https://claude.com/claude-code).
