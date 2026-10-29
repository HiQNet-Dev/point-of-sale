#!/usr/bin/env python3
"""Verifica evidencias del laboratorio sin aceptar un verde normativo ficticio."""

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

FORBIDDEN = (
    re.compile(r"\$2[aby]\$"),
    re.compile(r"\$argon2"),
    re.compile(r"\$2[aby]\$[0-9]{2}\$"),
)


def load(path):
    with path.open(encoding="utf-8") as stream:
        return json.load(stream)


def fail(message):
    print(f"verify-evidence: {message}", file=sys.stderr)
    raise SystemExit(1)


def verify_snapshot_digests(root):
    directory = root / "schema-snapshots"
    if directory.is_symlink():
        fail("snapshot directory must not be a symlink")
    manifest_path = directory / "technical-digests.json"
    if manifest_path.is_symlink():
        fail("snapshot digest manifest must not be a symlink")
    try:
        manifest = load(manifest_path)
    except (OSError, ValueError):
        fail("snapshot digest manifest missing or unreadable")
    snapshots = manifest.get("snapshots") if isinstance(manifest, dict) else None
    if not isinstance(snapshots, list):
        fail("snapshot digest manifest must contain a snapshot list")

    # El runner integrado publica estos siete snapshots; una lista truncada no
    # acredita integridad aunque los digests que sobrevivan sean correctos.
    expected = {
        f"schema-snapshots/{name}.json"
        for name in (
            "lab-v1",
            "lab-v2",
            "storage-local-subset",
            "postgres-central-clean",
            "postgres-control-clean",
            "postgres-central-v2",
            "postgres-control-v2",
        )
    }
    paths = [item.get("path") if isinstance(item, dict) else None for item in snapshots]
    if (
        len(paths) != len(expected)
        or any(not isinstance(path, str) for path in paths)
        or set(paths) != expected
    ):
        fail("snapshot digest manifest has missing, duplicate or unexpected paths")
    for item in snapshots:
        relative = item["path"]
        digest = item.get("sha256")
        if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
            fail(f"snapshot SHA-256 missing or invalid: {relative}")
        path = root / relative
        if path.is_symlink():
            fail(f"snapshot must not be a symlink: {relative}")
        try:
            observed = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError:
            fail(f"snapshot missing or unreadable: {relative}")
        if observed != digest:
            fail(f"snapshot SHA-256 mismatch: {relative}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--expect-exit", required=True, type=int, choices=(0, 1, 2))
    args = parser.parse_args()
    root = args.evidence
    if not root.is_dir():
        fail("evidence directory missing")

    parity = load(root / "parity-report.json")
    normative = parity.get("normative_cases", [])
    technical = parity.get("technical_checks", [])
    expected_ids = {
        *(f"DDL13-{index:02}" for index in range(1, 12)),
        *(f"PAR-{index:02}" for index in range(1, 10)),
        *(f"AS-{index:02}" for index in range(1, 11)),
    }
    if (
        not isinstance(normative, list)
        or len(normative) != len(expected_ids)
        or any(not isinstance(case, dict) for case in normative)
    ):
        fail("normative cases must be a list of 30 objects")
    # La cantidad por sí sola permite sustituir un caso faltante por un duplicado.
    identifiers = [case.get("id") for case in normative]
    if (
        any(not isinstance(identifier, str) for identifier in identifiers)
        or set(identifiers) != expected_ids
    ):
        fail("normative case IDs have missing, duplicate or unexpected values")
    if any(
        not isinstance(case.get("status"), str)
        or case["status"] not in {"PASSED", "BLOCKED", "NOT_RUN"}
        for case in normative
    ):
        fail("normative case has an invalid status")
    passed = {case["id"] for case in normative if case["status"] == "PASSED"}
    if passed != {"PAR-08"}:
        fail("only PAR-08 is approved at the source-exporter level")
    normative_approved = sum(case["status"] == "PASSED" for case in normative)
    # NOT_RUN sigue pendiente: no reduce el bloqueo normativo ni acredita paridad.
    normative_blocked = sum(
        case["status"] in {"BLOCKED", "NOT_RUN"} for case in normative
    )
    if any(
        type(parity.get(field)) is not int or parity[field] != observed
        for field, observed in (
            ("normative_approved", normative_approved),
            ("normative_blocked", normative_blocked),
        )
    ):
        fail("normative approval counters are inconsistent")
    if parity.get("normative_complete") is not False:
        fail("normative_complete must remain false")
    if parity.get("exit_code") != args.expect_exit:
        fail(f"expected exit {args.expect_exit}, observed {parity.get('exit_code')}")
    if parity.get("technical_status") != "PASSED":
        fail("technical checks are not PASSED")
    if any(case.get("status") != "PASSED" for case in technical):
        fail("a technical check failed")
    expected = 41
    if len(technical) != expected:
        fail(f"expected {expected} technical checks, observed {len(technical)}")

    auth = parity["external_labs"]["auth"]
    if [case.get("id") for case in auth["cases"]] != [
        f"AUTH-MIG-{index:02}" for index in range(1, 9)
    ]:
        fail("AUTH-MIG-01..08 missing or out of order")
    if auth["policy"]["commercial_approved"] is not False:
        fail("auth policy must not claim commercial approval")
    if auth["benchmark"]["host"]["product_hardware"] is not False:
        fail("benchmark must not claim product hardware")

    postgres = parity["external_labs"]["postgres"]
    if (
        postgres.get("technical_status") != "PASSED"
        or postgres.get("check_count") != 19
    ):
        fail("PostgreSQL 17 technical checks are incomplete")
    if postgres["cleanup"] != {
        "child_reaped": True,
        "pools_closed": True,
        "temporary_cluster_removed": True,
    }:
        fail("PostgreSQL cleanup was not verified")

    source = parity["external_labs"]["source"]
    if (
        source.get("status") != "PASSED"
        or source.get("target_import") != "BLOCKED_CONFLICT_FREEZE"
    ):
        fail("source lab status/import boundary invalid")
    observations = source.get("observations", [])
    if not any(
        item.get("id") == "PAR-08" and item.get("status") == "PASSED"
        for item in observations
    ):
        fail("PAR-08 source exporter determinism is not evidenced")
    installs = [
        item
        for item in observations
        if item.get("id", "").startswith("SOURCE-INSTALL-")
    ]
    if len(installs) != 2 or any(
        item.get("table_count") != 37 or item.get("populated_tables") != 37
        for item in installs
    ):
        fail("source coverage is not 37/37 on two installs")
    reconstruction = next(
        (item for item in observations if item.get("id") == "SOURCE-RECONSTRUCTION"),
        None,
    )
    if reconstruction is None or reconstruction.get("status") != "PASSED":
        fail("source reconstruction did not pass")
    if reconstruction.get("equal_functional_digests") is not True:
        fail("source reconstructions do not share a digest")
    if source.get("actions", {}).get("refresh_sessions") != "INVALIDATE":
        fail("Java sessions must be invalidated, never imported")

    counts = load(root / "counts.json")
    if counts.get("status") != "OBSERVED_SOURCE_ONLY":
        fail("counts are not marked source-only")
    source_counts = counts.get("source", {})
    if (
        source_counts.get("table_count") != 37
        or source_counts.get("populated_tables") != 37
    ):
        fail("counts do not show 37/37 source tables")
    if source_counts.get("refresh_sessions") != "INVALIDATE":
        fail("counts do not invalidate refresh sessions")

    money = load(root / "money-vectors.json")
    if money.get("status") != "TECHNICAL_OBSERVED":
        fail("money vectors are not technical observations")
    vectors = {(v["table"], v["column"], v["scale"]): v for v in money["vectors"]}
    sale_two = vectors[("sales", "total_amount", 2)]["values"][0]
    sale_four = vectors[("sales", "total_amount", 4)]["values"][0]
    if sale_two["result"]["status"] != "REJECTED":
        fail("12.3456 at MXN scale 2 must be rejected, not rounded")
    if sale_four["result"] != {"status": "EXACT", "minor": 123456, "currency": "MXN"}:
        fail("12.3456 at scale 4 is not exact")
    cost_values = vectors[("purchase_order_lines", "unit_cost", 4)]["values"]
    if cost_values[0]["result"]["status"] != "SQL_NULL":
        fail("NULL unit_cost was not preserved")
    if (
        cost_values[1]["result"]["status"] != "EXACT"
        or cost_values[1]["result"]["minor"] != 0
    ):
        fail("zero unit_cost is not distinct from NULL")

    stock = load(root / "stock-reconciliation.json")
    if stock.get("resulting_quantity_chain_valid") is not True:
        fail("stock resulting_quantity chain invalid")
    if not stock.get("balances") or not all(
        item.get("equal") for item in stock["balances"]
    ):
        fail("stock ledger does not reconcile with balances")

    baseline = load(root / "baseline-manifest.json")
    if baseline.get("status") != "VERIFIED_SOURCE_INVENTORY":
        fail("baseline manifest is not verified")
    if baseline.get("flyway", {}).get("observed_count") != 25:
        fail("Flyway count is not 25")
    if baseline.get("table_inventory", {}).get("observed_count") != 37:
        fail("AS-IS table count is not 37")

    verify_snapshot_digests(root)

    if any((root / name).exists() for name in ("bundle-1.json", "bundle-2.json")):
        fail("private source bundles must not be copied to public evidence")

    for path in root.rglob("*"):
        if path.suffix not in {".json", ".md", ".xml", ".txt"} or not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        for pattern in FORBIDDEN:
            if pattern.search(text):
                fail(f"credential-like material found in {path.relative_to(root)}")

    print(
        "verify-evidence: OK "
        f"(technical={len(technical)}, normative_passed={normative_approved}, "
        f"normative_blocked={normative_blocked}, "
        f"source_tables={source_counts['table_count']}, exit={args.expect_exit})"
    )


if __name__ == "__main__":
    main()
