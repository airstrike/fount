# Vendored fount

Source: [airstrike/fount](https://github.com/airstrike/fount), revision
`5bb7711197354b35160acdf0cb85862248a16e0f` (0.2.0).
The upstream MIT license is preserved in [LICENSE](LICENSE).

icedcn enables this crate through its optional `fount` feature. The
`icedcn::fount` module re-exports the provider and adds iced `Task` functions for
catalog discovery and font registration. Font bytes are downloaded only when a
family is selected; applications keep control of their chosen font.

Local changes:

- Native catalog requests retain fount's disk cache and fall back to the bundled
  catalog when networking or cache access fails. A valid older cache is
  supplemented with missing bundled families while retaining its existing
  family metadata and any newer live-only entries.
- Web builds use the same catalog model, search API, and error types. They read
  the complete bundled catalog snapshot and download TrueType/OpenType files
  from Google's font repository or `fonts.gstatic.com`, both of which permit
  cross-origin reads. Google's live metadata endpoint does not permit browser
  cross-origin reads, and its browser CSS responses use WOFF2, which iced's font
  parser does not accept.
- Native configuration/system discovery dependencies are compiled only on native
  targets. Google metadata category names accept the current spelling as well
  as upstream's older names. HTTP failures are checked before parsing.
- `Fount::search` searches every known family. A UI may cap visible results after
  filtering without limiting the searchable catalog.

## Catalog snapshot

[data/provenance.json](data/provenance.json) records the refresh date, family
count, and pinned Google Fonts repository revision. The 2026-11-15 snapshot
contains all 1,961 families from Google's metadata response. Six families absent
from the repository use official Google CSS-derived TrueType URLs. The refresh
script checks their font signatures and CORS response headers.

From the icedcn repository root, run:

```sh
python3 scripts/sync-google-fonts.py
```

The script fetches Google's current metadata and full repository tree, builds a
complete index, and resolves any missing repository families through Google's
CSS endpoint. It fails if a family lacks downloadable supported fonts; it does
not silently omit families. No API key or custom server is required.

Web catalog discovery works offline from the snapshot. Downloading a family
still requires network access unless the browser has cached its files. The web
catalog updates with an icedcn release containing a refreshed snapshot; it is
not advertised as a live catalog. Native catalog discovery normally refreshes
Google's metadata after seven days.

Font files retain their individual upstream licenses. This package bundles
catalog metadata and source URLs, not font binaries. Google repository fonts
and their licenses are available at the revision recorded in provenance.
