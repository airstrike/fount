<div align="center">

# fount

A font manager for [iced](https://github.com/iced-rs/iced) applications

[![Made with iced](https://iced.rs/badge.svg)](https://github.com/iced-rs/iced)

</div>

## Overview

`fount` is a font manager for `iced` applications. It discovers system fonts,
fetches families from Google Fonts on demand, and loads custom URL-hosted
fonts — all unified behind a single `Fount` handle and configured from a
`fonts.toml` file or built up programmatically.

Loading is async and disk-cached. Google Fonts are fetched over the public
CSS2 endpoint (no API key needed) and stored under `{cache_dir}/fount/google/`
so subsequent runs are offline-friendly.

> **Note:** `fount` tracks `iced` from git. It depends on `iced_core` 0.15.0-dev
> and is not yet published to crates.io.

## Installation

Add `fount` as a git dependency:

```toml
[dependencies]
fount = { git = "https://github.com/airstrike/fount" }
```

Enable the `office` feature on machines with Microsoft Office to also surface
its bundled fonts (Aptos, Calibri, Cambria, …):

```toml
fount = { git = "https://github.com/airstrike/fount", features = ["office"] }
```

## Features

### System font discovery

Walks the platform's font directories, parses each TTF/OTF/TTC, and returns a
deduped list of `(family, style, path)` records. macOS uses a curated allowlist
by default (`MACOS_SANE_FONTS`) so menus aren't drowned in symbol fonts;
Linux and Windows scan unfiltered.

```rust
let fonts = fount::system::discover(&fount::system::Config::default());
for f in &fonts {
    println!("{} {}", f.family, f.style);
}
```

`discover` is blocking (directory traversal + font parsing) — wrap it in
`tokio::task::spawn_blocking` from async contexts, as the picker example does.

### Google Fonts catalog and on-demand loading

Fetch the full catalog (cached on disk for 7 days by default) and then load
individual families by name:

```rust
use fount::google;

let catalog = google::catalog(google::DEFAULT_CATALOG_MAX_AGE).await?;

// Load just the variants this family actually publishes
let bytes = google::load("Inter", Some(&catalog)).await?;

// Or request specific weights/italics directly
let bytes = google::load_variants("Inter", &["400", "700", "400i"]).await?;
```

Variant keys follow Google's CSS2 conventions: `"400"`, `"700"`, `"400i"`,
`"700i"`, etc. Variable-range files (`100..900.ttf`) are detected in the cache
so a request for `"400"` won't redownload when a `100..900.ttf` already covers
it.

### Custom fonts from URLs

Drop arbitrary font URLs into the config and they're fetched and cached the
same way as Google Fonts.

```toml
[[custom]]
name = "My Brand Font"
url = "https://example.com/fonts/brand.ttf"
variants = ["400", "700"]
```

### `fonts.toml` configuration

A single TOML file declares system filtering, Google Fonts behavior, and
custom font sources. See [`fonts.example.toml`](fonts.example.toml) for the
full schema.

```toml
[system]
include = ["Helvetica Neue", "Menlo", "SF Pro"]
exclude = ["Apple Symbols"]

[google]
enabled = true
preload = ["Inter"]
catalog_limit = 100
```

### Office font support (opt-in)

The `office` cargo feature adds Microsoft Office's private font directories
(macOS `DFonts`, Windows `VFS\Fonts\private`) to the default scan list, which
surfaces fonts that ship inside Office apps but not in the OS font folders —
Aptos, Calibri, Cambria, Consolas, and friends. Only enable on machines with a
valid Office license.

There's also a Mac Roman name-table fallback in the parser so Aptos (which
only carries Macintosh-platform name records) is decoded correctly instead of
silently dropped.

## Usage

```rust
use fount::Fount;

let mut fount = Fount::new();

// Populate from whichever sources you want
fount.set_system_families(
    fount::system::family_names(&fount::system::discover(&Default::default()))
);
fount.set_google_catalog(
    fount::google::catalog(fount::google::DEFAULT_CATALOG_MAX_AGE).await?
);

// Unified queries across every source
let all = fount.families();           // sorted, deduped
assert!(fount.has_family("Inter"));

// Get an iced Font descriptor (the bytes are loaded separately)
let font = fount.font("Inter");
```

`Fount` itself is just an aggregator — it does not register fonts with iced
on your behalf. Once you have raw bytes from `google::load`, `system::load`,
or your own source, hand them to `iced::font::load()` and use the descriptor
returned by `Fount::font` in your views.

## Examples

```bash
cargo run -p picker                    # combo-box font picker, system + Google
cargo run -p picker --features office  # also surface Office-bundled fonts
```

The picker example demonstrates the full loading flow: parallel system
discovery and Google catalog fetch, async per-family registration, and a
braille-dot spinner that runs while the renderer catches up to the user's
selection.

## Code Structure

- `src/lib.rs` — the `Fount` aggregator and the unified family-query API
- `src/config.rs` — `fonts.toml` schema and loader
- `src/system.rs` — platform font directory scanning, OpenType name parsing,
  curated macOS allowlist, optional Office paths
- `src/google/` — catalog parsing, CSS2 fetcher, variant key handling, disk
  cache, variable-range cache hits
- `src/error.rs` — the unified `Error` type

## Contributing

Contributions are welcome. Bug reports, feature ideas, and PRs all land in the
same place — open an issue or PR on
[GitHub](https://github.com/airstrike/fount).

## License

MIT

## Acknowledgements

- [iced](https://github.com/iced-rs/iced)
- [Google Fonts](https://fonts.google.com/)
- [Rust programming language](https://www.rust-lang.org/)
