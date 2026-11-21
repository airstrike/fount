#!/usr/bin/env python3
"""Refresh fount's complete Google Fonts metadata and CORS-safe TrueType index."""
import concurrent.futures
import datetime
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[1] / "data"


def fetch(url, user_agent="fount-font-catalog"):
    request = urllib.request.Request(url, headers={"User-Agent": user_agent})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def missing_sources(family):
    # Legacy CSS clients receive TrueType, which iced can parse directly.
    # Browser user agents receive WOFF2 instead and cannot choose this header.
    name = family["family"]
    variants = family["fonts"].keys()
    italic = any(value.endswith("i") for value in variants)
    entries = sorted({(int(value.endswith("i")), int(value.rstrip("i"))) for value in variants})
    axis = "ital,wght" if italic else "wght"
    weights = ";".join(f"{i},{w}" if italic else str(w) for i, w in entries)
    encoded = urllib.parse.quote_plus(name)
    css = fetch(f"https://fonts.googleapis.com/css2?family={encoded}:{axis}@{weights}", "Mozilla/4.0").decode()
    urls = list(dict.fromkeys(re.findall(r"url\(([^)]+)\)", css)))
    if not urls:
        raise RuntimeError(f"No TrueType sources for {name}")
    for url in urls:
        request = urllib.request.Request(url, headers={"Origin": "https://icedcn.dev"})
        with urllib.request.urlopen(request, timeout=60) as response:
            if response.headers.get("Access-Control-Allow-Origin") != "*":
                raise RuntimeError(f"Font URL does not permit cross-origin reads: {url}")
            if response.read(4) not in (b"\x00\x01\x00\x00", b"OTTO", b"ttcf"):
                raise RuntimeError(f"Unsupported font format: {url}")
    return name, urls


def main():
    raw = fetch("https://fonts.google.com/metadata/fonts").decode().removeprefix(")]}'").lstrip()
    metadata = json.loads(raw)["familyMetadataList"]
    tree = json.loads(fetch("https://api.github.com/repos/google/fonts/git/trees/main?recursive=1"))
    if tree.get("truncated"):
        raise RuntimeError("Google Fonts source tree was truncated")
    revision = tree["sha"]
    paths = {}
    for entry in tree["tree"]:
        path = entry["path"]
        parts = path.split("/")
        if len(parts) == 3 and parts[0] in ("ofl", "apache", "ufl") and path.endswith((".ttf", ".otf")):
            paths.setdefault(parts[1], []).append(path)
    sources = {}
    missing = []
    for family in metadata:
        name = family["family"]
        directory = re.sub(r"[^a-z0-9]", "", name.lower())
        if directory in paths:
            sources[name] = sorted(paths[directory])
        else:
            missing.append(family)
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
        for name, urls in executor.map(missing_sources, missing):
            sources[name] = urls
    # Keep only provider fields rather than unrelated Google website metadata.
    fields = ("family", "category", "popularity", "axes", "isNoto")
    compact = [{**{key: f[key] for key in fields if key in f}, "fonts": {key: {} for key in f["fonts"]}} for f in metadata]
    ROOT.mkdir(parents=True, exist_ok=True)
    (ROOT / "metadata.json").write_text(json.dumps({"familyMetadataList": compact}, ensure_ascii=False, separators=(",", ":")) + "\n")
    base = f"https://raw.githubusercontent.com/google/fonts/{revision}/"
    (ROOT / "catalog.tsv").write_text("# " + base + "\n" + "".join(name + "\t" + "\t".join(sources[name]) + "\n" for name in sorted(sources)))
    provenance = {"refreshed": datetime.date.today().isoformat(), "families": len(metadata), "google_fonts_revision": revision, "metadata": "https://fonts.google.com/metadata/fonts", "css_fallback_families": len(missing)}
    (ROOT / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    print(json.dumps(provenance))


if __name__ == "__main__":
    main()
