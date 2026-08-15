# Zerlak Frontier — web front end

A small [Vite](https://vitejs.dev/) + React shell that hosts the game's
WebAssembly build in a letterboxed canvas. See the [repo root
README](../README.md) for how to build and deploy the whole thing — this
directory alone isn't runnable, since `src/wasm/` (the game itself) is
generated from the Rust build rather than checked in.

```sh
npm install
npm run dev      # local dev server, expects src/wasm/ to already exist
npm run build    # production build -> dist/
```
