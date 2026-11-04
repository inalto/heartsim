# Building this guide

The guide is an [mdBook](https://rust-lang.github.io/mdBook/) in `docs/book/`. Install mdBook once:

```bash
cargo install mdbook --locked
```

Then build it, or serve it with live reload at <http://localhost:3000>:

```bash
mdbook build docs/book
```

```bash
mdbook serve docs/book
```

The output goes to `docs/book/book/` (git-ignored). `docs/book/src/images` is a link to `docs/images/`, so the guide uses the
same screenshots as the README; regenerate them with the screenshot run in
[Running tests and benchmarks](benchmarks.md#screenshots-for-the-documentation).

The `docs` workflow (`.github/workflows/docs.yml`) builds the book on every pull request that touches `docs/`, fails on any
mdBook warning, and publishes it to GitHub Pages from `master`.
