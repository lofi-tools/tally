#!/usr/bin/env python3
"""Generate the embedded UK taxonomy concept table used by the validate-uk crate.

The checker needs per-concept metadata (data type, periodType, balance,
enumeration values, pattern / minLength facets) to run schema-level checks.
That metadata lives in the FRC / HMRC XSDs, which are not vendored in this
repo (they are downloaded on demand, e.g. by the flake).  This script
embeds the **full ct-comp computation taxonomy** (every concept, so
computation documents from any tool are schema-checked) plus the FRC
concepts the report generators emit, and writes a compact JSON table that is
embedded in the crate via `include_str!`.

Regenerate after the taxonomy version changes, or when the generators start
emitting new FRC concepts:

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
  - languages.xsd    (uk-lang language dimension + members)
  - dpl.xsd          (dpl computation line items)
  - ct-comp.xsd      (ct-comp computation concepts)
  - ct-comp-types-2023.xsd  (ct-comp type facets: tax reference / district
                             patterns, non-empty string minLength)
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
    # Named type facets: type local name -> (pattern, minLength).  Extracted
    # from the ct-comp types schema (taxReferenceItemType etc.) in a first
    # pass, then attached to the elements that reference those types.
    facets: dict[str, tuple[str | None, int | None]] = {}

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
        if kw.get("pat"):
            d["pat"] = kw["pat"]
        if kw.get("ml") is not None:
            d["ml"] = kw["ml"]
        concepts.setdefault(name, d)

    def collect_type_facets(root) -> None:
        """Record pattern / minLength facets for named simple and complex
        types (the ct-comp types schema defines taxReferenceItemType,
        taxDistrictItemType, nonEmptyStringItemType, nonEmptyString)."""
        for el in root.iter():
            tag = qname(el.tag)
            name = el.attrib.get("name")
            if not name:
                continue
            if tag not in ("simpleType", "complexType"):
                continue
            restriction = None
            if tag == "simpleType":
                restriction = el.find(f"{XS}restriction")
            else:
                sc = el.find(f"{XS}simpleContent")
                restriction = sc.find(f"{XS}restriction") if sc is not None else None
            if restriction is None:
                continue
            pattern = None
            min_len = None
            for c in restriction:
                ct = qname(c.tag)
                if ct == "pattern":
                    pattern = c.attrib.get("value")
                elif ct == "minLength":
                    v = c.attrib.get("value")
                    if v:
                        min_len = int(v)
            if pattern or min_len is not None:
                # Keyed by the type's own name (taxReferenceItemType etc.):
                # elements reference these by name via their `type` attribute.
                facets[name] = (pattern, min_len)

    # Global first pass: named-type facets, across every file, before any
    # element is processed (the ct-comp types file lists after ct-comp-2.xsd,
    # but its elements reference those types).
    for path in xsd_paths:
        collect_type_facets(ET.parse(path).getroot())

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
                pattern, min_len = facets.get(kind, (None, None))
                add(name, period=period, balance=balance, nillable=nillable,
                    typ=kind, enum=enums or None, pat=pattern, ml=min_len)
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
        xsd_dir / "languages.xsd",
        xsd_dir / "dpl.xsd",
        xsd_dir / "ct-comp-2.xsd",
        xsd_dir / "ct-comp-types-2023.xsd",
    ]
    missing = [p for p in xsd_files if not p.exists()]
    if missing:
        raise SystemExit(f"missing XSDs: {', '.join(str(p) for p in missing)}")

    concepts = collect_concepts(xsd_files)
    used = used_concepts([Path(f) for f in args.fixtures])

    # Welsh reports use the language dimension + report-language marker even
    # though the English fixtures never reference them — keep them and their
    # types.
    used |= {"Welsh", "LanguagesDimension", "ReportPrincipalLanguage"}

    # The full ct-comp computation taxonomy is embedded, not just the subset
    # the generators emit: the checker validates computation documents
    # produced by any tool, so every ct-comp concept must be known.  (The FRC
    # accounts concepts stay fixture-filtered — the generators only emit a
    # small slice of the full FRS-2022 taxonomy.)
    ct_comp = set()
    ct_root = ET.parse(Path(args.xsd_dir) / "ct-comp-2.xsd").getroot()
    for el in ct_root.iter():
        if qname(el.tag) == "element" and el.attrib.get("name"):
            ct_comp.add(el.attrib["name"])
    used |= ct_comp

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
