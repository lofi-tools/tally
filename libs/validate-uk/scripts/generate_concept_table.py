#!/usr/bin/env python3
"""Generate the embedded UK taxonomy concept table used by the validate-uk crate.

The checker needs per-concept metadata (data type, periodType, balance,
enumeration values) to run schema-level checks.  That metadata lives in the
FRC / HMRC XSDs, which are not vendored in this repo (they are downloaded on
demand, e.g. by the flake).  This script extracts the subset of concepts that
the report generators actually emit, plus the types they reference, and writes
a compact JSON table that is embedded in the crate via `include_str!`.

Regenerate after the generators start emitting new concepts, or when the
taxonomy version changes:

    python3 libs/validate-uk/scripts/generate_concept_table.py \
        --xsd-dir /path/to/taxonomy \
        --fixtures example_data/basic-1/output-accounts.html \
                   example_data/basic-1/output-corp-tax.html \
        --out libs/validate-uk/src/taxonomy/uk-2023-01-01.json

The XSDs needed (all 2023-01-01):
  - bus.xsd          (uk-bus)
  - frc-core.xsd     (uk-core, downloaded from .../fr/2023-01-01/core/)
  - types.xsd        (FRC general types, incl. fixedItemType)
  - direp.xsd        (uk-direp statement concepts)
  - countries.xsd    (uk-geo)
  - dpl.xsd          (dpl computation line items)
  - ct-comp.xsd      (ct-comp computation concepts)
"""
from __future__ import annotations

import argparse
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path

XS = "{http://www.w3.org/2001/XMLSchema}"


def qname(tag: str) -> str:
    return tag.split("}")[-1] if "}" in tag else tag


def _attr_local(el, local: str):
    """Look up an attribute by local name (handles Clark-notation keys like
    `{http://www.xbrl.org/2003/instance}periodType`)."""
    for key, value in el.attrib.items():
        if qname(key) == local:
            return value
    return None


def collect_concepts(xsd_paths: list[Path]) -> dict[str, dict]:
    """Extract element and simpleType definitions from a set of XSD files."""
    concepts: dict[str, dict] = {}

    def add(name: str, **kw) -> None:
        d = {"name": name}
        if kw.get("period"):
            d["p"] = kw["period"]
        if kw.get("balance"):
            d["b"] = kw["balance"]
        if kw.get("nillable"):
            d["n"] = True
        if kw.get("typ"):
            d["t"] = kw["typ"]
        if kw.get("enum"):
            d["e"] = kw["enum"]
        concepts.setdefault(name, d)

    for path in xsd_paths:
        root = ET.parse(path).getroot()
        for el in root.iter():
            tag = qname(el.tag)
            if tag == "element":
                name = el.attrib.get("name")
                if not name:
                    continue  # local element ref inside a complexType
                typ = el.attrib.get("type")
                # The periodType / balance attributes are namespaced in the
                # FRC/HMRC schemas (`xbrli:periodType="instant"`).  ElementTree
                # expands namespaced attributes to Clark notation, so match by
                # local name.
                period = _attr_local(el, "periodType")
                balance = _attr_local(el, "balance")
                nillable = el.attrib.get("nillable") == "true"
                enums: list[str] = []
                # inline simpleType with enumeration restriction
                st = el.find(f"{XS}simpleType")
                if st is not None:
                    rest = st.find(f"{XS}restriction")
                    if rest is not None:
                        for e in rest.findall(f"{XS}enumeration"):
                            v = e.attrib.get("value")
                            if v is not None:
                                enums.append(v)
                        if enums:
                            typ = rest.attrib.get("base") or "enum"
                kind = typ.rpartition(":")[2] if typ else None
                add(name, period=period, balance=balance, nillable=nillable,
                    typ=kind, enum=enums or None)
            elif tag == "simpleType":
                name = el.attrib.get("name")
                if not name:
                    continue
                rest = el.find(f"{XS}restriction")
                if rest is None:
                    continue
                base = rest.attrib.get("base")
                enums = [e.attrib.get("value") for e in rest.findall(f"{XS}enumeration")]
                enums = [e for e in enums if e is not None]
                if enums:
                    add(name, typ=base or "simple", enum=enums)
                else:
                    add(name, typ=base or "simple")
    return concepts


def used_concepts(fixtures: list[Path]) -> set[str]:
    """Collect the concept local names referenced by the given iXBRL fixtures."""
    used: set[str] = set()
    for path in fixtures:
        html = path.read_text()
        used.update(re.findall(r'name="[^":]+:([^"]+)"', html))
        used.update(re.findall(r"<xbrldi:explicitMember[^>]*>([^<]+)</xbrldi:explicitMember>", html))
        for m in re.findall(r"<xbrldi:explicitMember[^>]*>([^<]+)</xbrldi:explicitMember>", html):
            used.add(m.rpartition(":")[2] if ":" in m else m)
    return used


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--xsd-dir", required=True, help="directory containing the downloaded XSDs")
    ap.add_argument("--fixtures", nargs="+", required=True, help="generated iXBRL fixtures to cover")
    ap.add_argument("--out", required=True, help="output JSON path")
    args = ap.parse_args()

    xsd_dir = Path(args.xsd_dir)
    xsd_files = [
        xsd_dir / "bus.xsd",
        xsd_dir / "frc-core.xsd",
        xsd_dir / "types.xsd",
        xsd_dir / "direp.xsd",
        xsd_dir / "countries.xsd",
        xsd_dir / "dpl.xsd",
        xsd_dir / "ct-comp-2.xsd",
    ]
    missing = [p for p in xsd_files if not p.exists()]
    if missing:
        raise SystemExit(f"missing XSDs: {', '.join(str(p) for p in missing)}")

    concepts = collect_concepts(xsd_files)
    used = used_concepts([Path(f) for f in args.fixtures])

    # Also keep the types the used concepts reference (e.g. fixedItemType).
    keep = set(used)
    for name in used:
        d = concepts.get(name)
        if d and d.get("t"):
            keep.add(d["t"])

    table = {name: d for name, d in concepts.items() if name in keep}
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(table, indent=1, sort_keys=True) + "\n")
    print(f"wrote {len(table)} concepts ({out.stat().st_size} bytes) to {out}")


if __name__ == "__main__":
    main()
