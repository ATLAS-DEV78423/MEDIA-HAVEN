#!/usr/bin/env python3
"""Offline third-party licence inventory for MEDIA HAVEN.

Reads only Cargo.lock, apps/desktop/package-lock.json and the local Cargo
registry cache. Installs nothing, fetches nothing and needs no network.
Regenerate with: python infrastructure/packaging/windows/collect_licenses.py
"""
from __future__ import annotations

import json
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent / "THIRD_PARTY_NOTICES.md"
REGISTRY_SRC = Path.home() / ".cargo" / "registry" / "src"


def rust_components() -> list[tuple[str, str, str]]:
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    registry = [
        p for p in lock.get("package", [])
        if str(p.get("source", "")).startswith("registry+")
    ]

    declared: dict[str, str] = {}
    for manifest in REGISTRY_SRC.glob("*/*/Cargo.toml"):
        try:
            pkg = tomllib.loads(manifest.read_text(encoding="utf-8")).get("package", {})
        except (OSError, tomllib.TOMLDecodeError):
            continue
        licence = pkg.get("license")
        if isinstance(licence, str) and licence.strip():
            declared[manifest.parent.name] = licence.strip()

    rows = []
    for p in registry:
        key = f"{p['name']}-{p['version']}"
        rows.append((p["name"], p["version"], declared.get(key, "unresolved offline")))
    return sorted(rows, key=lambda r: (r[0].lower(), r[1]))


def node_components() -> list[tuple[str, str, str]]:
    lock = json.loads((ROOT / "apps/desktop/package-lock.json").read_text(encoding="utf-8"))
    rows = []
    for path, meta in lock.get("packages", {}).items():
        if not path or not meta.get("version"):
            continue  # root package / link entries
        licence = meta.get("license", "unresolved")
        if isinstance(licence, dict):
            licence = licence.get("type", "unresolved")
        name = path.split("node_modules/")[-1]
        rows.append((name, str(meta["version"]), str(licence)))
    return sorted(rows, key=lambda r: (r[0].lower(), r[1]))


def table(rows: list[tuple[str, str, str]]) -> list[str]:
    lines = ["| Component | Version | Declared licence |", "|---|---|---|"]
    lines += [f"| {n} | {v} | {lic} |" for n, v, lic in rows]
    return lines


def main() -> None:
    rust = rust_components()
    node = node_components()
    unresolved_rust = sum(1 for r in rust if r[2] == "unresolved offline")
    unresolved_node = sum(1 for r in node if r[2] == "unresolved")

    lines = [
        "# Third-party notices - MEDIA HAVEN desktop",
        "",
        "Deterministically generated from the lockfiles by",
        "`infrastructure/packaging/windows/collect_licenses.py` (no date stamp: reproducible).",
        "Do not hand-edit: re-run the script after any lockfile change.",
        "",
        "Licence strings are the values **declared** by each dependency's upstream",
        "manifest or lockfile. They are a notice, not legal advice. Full licence texts",
        "ship inside the dependency sources (Cargo registry cache / `node_modules`) and",
        "must be included in any source distribution that redistributes them.",
        "",
        f"## Rust / Cargo - {len(rust)} registry crates ({unresolved_rust} unresolved offline)",
        "",
    ]
    lines += table(rust)
    lines += [
        "",
        f"## Node / npm - {len(node)} packages ({unresolved_node} unresolved)",
        "",
    ]
    lines += table(node)
    lines += [
        "",
        "## Media metadata attribution - PLACEHOLDER",
        "",
        "NOT YET PROVIDED. IMDb and TMDB attribution / licence notices are added by a",
        "separate agent under the IMDb metadata plan. This section is intentionally",
        "left unfilled; do not ship a release with it still empty.",
        "",
        "- IMDb: [attribution pending]",
        "- TMDB: [attribution pending - TMDb terms require displayed attribution]",
        "",
        "## Notes",
        "",
        "- `unresolved offline` means the crate's manifest was not in the local Cargo",
        "  registry cache; `cargo metadata --offline` cannot resolve it without a fetch.",
        "  Resolve these with an approved network step before publishing a release.",
        "- Workspace crates are local and excluded; they are covered by the repository",
        "  licence (MIT per `Cargo.toml`).",
        "- See `docs/operations/release.md` for how these notices are bundled.",
        "",
    ]
    OUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"Wrote {OUT.relative_to(ROOT)}: {len(rust)} Rust crates, {len(node)} npm packages")
    print(f"Unresolved: Rust={unresolved_rust}, npm={unresolved_node}")


if __name__ == "__main__":
    main()
