# B21 — WebAssembly build
**Crates** new `heartsim-web` (trunk entry), `heartsim-app` (feature gates) · **Model** Sonnet · **Milestone** M6 · **Depends on** M2.

## Read first
`docs/research/rust-stack.md` §5.5 (trunk, wasm-bindgen pinning, WebGPU/WebGL2 availability), §1.3 WebGL2 limits.

## Scope
1. `heartsim-web` with `index.html`, `trunk` config, `wasm-bindgen-futures` start, canvas sizing, loading screen.
2. Feature gates: no `kira` audio (use web audio via `web-sys` beep), no PDF, Tier 2 CPU only (no threads unless
   `wasm-bindgen-rayon` + COOP/COEP headers), anatomy pack fetched lazily (gzip), persistence via `localStorage` (eframe).
3. WebGL2 fallback: ensure shaders avoid storage buffers in VS and `@interpolate(linear)`.
4. GitHub Pages deploy workflow; size budget < 25 MB (wasm + pack).

## Acceptance tests
`trunk build --release` in CI; Playwright/`wasm-bindgen-test` smoke test that the canvas renders and the strip updates; manual test
matrix Chrome/Firefox/Safari documented.
