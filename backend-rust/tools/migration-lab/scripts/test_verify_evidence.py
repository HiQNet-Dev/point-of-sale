"""Regresiones del verificador con evidencias exclusivamente sintéticas."""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("verify_evidence.py")
SNAPSHOTS = (
    "lab-v1.json",
    "lab-v2.json",
    "storage-local-subset.json",
    "postgres-central-clean.json",
    "postgres-control-clean.json",
    "postgres-central-v2.json",
    "postgres-control-v2.json",
)


class VerifyEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        normative_ids = [
            *(f"DDL13-{index:02}" for index in range(1, 12)),
            *(f"PAR-{index:02}" for index in range(1, 10)),
            *(f"AS-{index:02}" for index in range(1, 11)),
        ]
        self.parity = {
            "normative_cases": [
                {
                    "id": identifier,
                    "status": "PASSED"
                    if identifier == "PAR-08"
                    else ("NOT_RUN" if identifier == "DDL13-10" else "BLOCKED"),
                }
                for identifier in normative_ids
            ],
            "normative_approved": 1,
            "normative_blocked": 29,
            "normative_complete": False,
            "normative_status": "BLOCKED",
            "exit_code": 0,
            "require_complete": False,
            "technical_status": "PASSED",
            "technical_checks": [
                {"id": f"SYNTHETIC-{index}", "status": "PASSED"} for index in range(41)
            ],
            "external_labs": {
                "auth": {
                    "cases": [{"id": f"AUTH-MIG-{index:02}"} for index in range(1, 9)],
                    "policy": {"commercial_approved": False},
                    "benchmark": {"host": {"product_hardware": False}},
                },
                "postgres": {
                    "technical_status": "PASSED",
                    "check_count": 19,
                    "cleanup": {
                        "child_reaped": True,
                        "pools_closed": True,
                        "temporary_cluster_removed": True,
                    },
                },
                "source": {
                    "status": "PASSED",
                    "target_import": "BLOCKED_CONFLICT_FREEZE",
                    "actions": {"refresh_sessions": "INVALIDATE"},
                    "observations": [
                        {"id": "PAR-08", "status": "PASSED"},
                        *(
                            {
                                "id": f"SOURCE-INSTALL-{index}",
                                "table_count": 37,
                                "populated_tables": 37,
                            }
                            for index in (1, 2)
                        ),
                        {
                            "id": "SOURCE-RECONSTRUCTION",
                            "status": "PASSED",
                            "equal_functional_digests": True,
                        },
                    ],
                },
            },
        }
        self.write_json("parity-report.json", self.parity)
        self.write_json(
            "counts.json",
            {
                "status": "OBSERVED_SOURCE_ONLY",
                "source": {
                    "table_count": 37,
                    "populated_tables": 37,
                    "refresh_sessions": "INVALIDATE",
                },
            },
        )
        self.write_json(
            "money-vectors.json",
            {
                "status": "TECHNICAL_OBSERVED",
                "vectors": [
                    {
                        "table": "sales",
                        "column": "total_amount",
                        "scale": 2,
                        "values": [{"result": {"status": "REJECTED"}}],
                    },
                    {
                        "table": "sales",
                        "column": "total_amount",
                        "scale": 4,
                        "values": [
                            {
                                "result": {
                                    "status": "EXACT",
                                    "minor": 123456,
                                    "currency": "MXN",
                                }
                            }
                        ],
                    },
                    {
                        "table": "purchase_order_lines",
                        "column": "unit_cost",
                        "scale": 4,
                        "values": [
                            {"result": {"status": "SQL_NULL"}},
                            {"result": {"status": "EXACT", "minor": 0}},
                        ],
                    },
                ],
            },
        )
        self.write_json(
            "stock-reconciliation.json",
            {
                "resulting_quantity_chain_valid": True,
                "balances": [{"equal": True}],
            },
        )
        self.write_json(
            "baseline-manifest.json",
            {
                "status": "VERIFIED_SOURCE_INVENTORY",
                "flyway": {"observed_count": 25},
                "table_inventory": {"observed_count": 37},
            },
        )
        self.write_execution_summaries()
        (self.root / "schema-snapshots").mkdir()
        self.manifest = {
            "scope": "synthetic_technical_or_storage_subset",
            "exit_code": 0,
            "snapshots": [],
        }
        for name in SNAPSHOTS:
            relative = f"schema-snapshots/{name}"
            self.write_json(relative, {"synthetic_snapshot": name})
            self.manifest["snapshots"].append(
                {
                    "path": relative,
                    "sha256": hashlib.sha256(
                        (self.root / relative).read_bytes()
                    ).hexdigest(),
                }
            )
        self.write_manifest()

    def write_json(self, relative, value):
        (self.root / relative).write_text(json.dumps(value), encoding="utf-8")
        if (
            relative == "parity-report.json"
            and (self.root / "baseline-manifest.json").is_file()
        ):
            self.write_execution_summaries()

    def write_manifest(self):
        self.write_json("schema-snapshots/technical-digests.json", self.manifest)

    def write_execution_summaries(self):
        baseline_digest = hashlib.sha256(
            (self.root / "baseline-manifest.json").read_bytes()
        ).hexdigest()
        complete = self.parity["require_complete"]
        code = self.parity["exit_code"]
        technical_status = self.parity["technical_status"]
        technical_failed = technical_status != "PASSED" or any(
            case["status"] != "PASSED" for case in self.parity["technical_checks"]
        )
        gate = (
            "NOT_EVALUATED"
            if technical_failed
            else ("BLOCKED" if complete else "NOT_REQUESTED")
        )
        self.write_json(
            "run-result.json",
            {
                "schema_version": 1,
                "technical_status": technical_status,
                "normative_status": "BLOCKED",
                "normative_complete": False,
                "normative_approved": 1,
                "normative_blocked": 29,
                "require_complete": complete,
                "exit_code": code,
                "baseline_evidence": {
                    "path": "baseline-manifest.json",
                    "sha256": baseline_digest,
                },
            },
        )
        self.write_json(
            "aggregate-status.json",
            {
                "schema_version": 1,
                "status": "FAILED" if technical_failed else "BLOCKED",
                "technical_status": technical_status,
                "normative_status": "BLOCKED",
                "baseline_evidence_status": "VERIFIED_SOURCE_INVENTORY",
                "gate": {
                    "require_complete": complete,
                    "status": gate,
                    "exit_code": code,
                },
                "sources": {
                    "run_result": "run-result.json",
                    "parity_report": "parity-report.json",
                    "baseline_manifest": "baseline-manifest.json",
                },
            },
        )

    def verify(self, exit_code=0):
        return subprocess.run(
            [
                sys.executable,
                "-B",
                str(SCRIPT),
                "--evidence",
                str(self.root),
                "--expect-exit",
                str(exit_code),
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )

    def assert_rejected(self, diagnostic="snapshot"):
        result = self.verify()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertNotIn("verify-evidence: OK", result.stdout)
        self.assertIn("verify-evidence:", result.stderr)
        self.assertIn(diagnostic, result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_valid_evidence_preserves_normal_and_incomplete_gate_results(self):
        for require_complete, exit_code in ((False, 0), (True, 2)):
            with self.subTest(require_complete=require_complete):
                self.parity["require_complete"] = require_complete
                self.parity["exit_code"] = exit_code
                self.write_json("parity-report.json", self.parity)
                self.write_execution_summaries()
                self.manifest["exit_code"] = exit_code
                self.write_manifest()
                result = self.verify(exit_code)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("normative_passed=1, normative_blocked=29", result.stdout)
                self.assertIn(f"exit={exit_code}", result.stdout)

    def test_inconsistent_execution_summaries_are_rejected(self):
        self.write_json("run-result.json", {"schema_version": 1})
        self.assert_rejected("run result")

    def test_technical_failure_has_exit_one_and_gate_not_evaluated(self):
        for require_complete in (False, True):
            with self.subTest(require_complete=require_complete):
                self.parity["require_complete"] = require_complete
                self.parity["technical_status"] = "FAILED"
                self.parity["technical_checks"][0]["status"] = "FAILED"
                self.parity["exit_code"] = 1
                self.write_json("parity-report.json", self.parity)
                result = self.verify(1)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_missing_normative_case_replaced_by_duplicate_is_rejected(self):
        original = self.parity["normative_cases"]
        for index, case in enumerate(original):
            if case["id"] == "PAR-08":
                continue
            with self.subTest(missing=case["id"]):
                cases = [dict(item) for item in original]
                replacement = "DDL13-02" if index == 0 else "DDL13-01"
                cases[index]["id"] = replacement
                self.parity["normative_cases"] = cases
                self.write_json("parity-report.json", self.parity)
                self.assert_rejected("normative case IDs")

    def test_unexpected_or_invalid_normative_ids_are_rejected(self):
        case = self.parity["normative_cases"][0]
        for identifier in (
            "DDL13-00",
            "DDL13-12",
            "PAR-00",
            "PAR-10",
            "AS-00",
            "AS-11",
            "DDL13-1",
            "ddl13-01",
            " DDL13-01",
            "SYNTHETIC-01",
            "",
            None,
            1,
            True,
            [],
            {},
        ):
            with self.subTest(identifier=identifier):
                case["id"] = identifier
                self.write_json("parity-report.json", self.parity)
                self.assert_rejected("normative case IDs")
        del case["id"]
        self.write_json("parity-report.json", self.parity)
        self.assert_rejected("normative case IDs")

    def test_malformed_normative_cases_are_rejected(self):
        original = self.parity["normative_cases"]
        for cases in (
            None,
            {},
            "x" * 30,
            [None] * 30,
            ["DDL13-01"] * 30,
            [*original[:-1], []],
        ):
            with self.subTest(cases=cases):
                self.parity["normative_cases"] = cases
                self.write_json("parity-report.json", self.parity)
                self.assert_rejected("normative case")

    def test_missing_and_extra_normative_cases_are_rejected(self):
        original = self.parity["normative_cases"]
        for cases in ([], original[:-1], [*original, original[0]]):
            with self.subTest(count=len(cases)):
                self.parity["normative_cases"] = cases
                self.write_json("parity-report.json", self.parity)
                self.assert_rejected("normative case")
        del self.parity["normative_cases"]
        self.write_json("parity-report.json", self.parity)
        self.assert_rejected("normative case")

    def test_invalid_or_missing_normative_status_is_rejected(self):
        case = self.parity["normative_cases"][0]
        for status in (None, "", "PASS", "FAILED", 1, True, [], {}):
            with self.subTest(status=status):
                case["status"] = status
                self.write_json("parity-report.json", self.parity)
                self.assert_rejected("normative case has an invalid status")
        del case["status"]
        self.write_json("parity-report.json", self.parity)
        self.assert_rejected("normative case has an invalid status")

    def test_par08_must_remain_the_only_passed_normative_case(self):
        cases = self.parity["normative_cases"]
        par08 = next(case for case in cases if case["id"] == "PAR-08")
        for par08_status, other_status in (
            ("BLOCKED", "BLOCKED"),
            ("NOT_RUN", "BLOCKED"),
            ("PASSED", "PASSED"),
            ("BLOCKED", "PASSED"),
        ):
            with self.subTest(par08=par08_status, other=other_status):
                par08["status"] = par08_status
                cases[0]["status"] = other_status
                self.write_json("parity-report.json", self.parity)
                self.assert_rejected("only PAR-08")

    def test_normative_counters_must_match_case_status_counts(self):
        for field, correct in (("normative_approved", 1), ("normative_blocked", 29)):
            for value in (None, 0, correct + 1, True, float(correct), str(correct)):
                with self.subTest(field=field, value=value):
                    self.parity[field] = value
                    self.write_json("parity-report.json", self.parity)
                    self.assert_rejected("normative approval counters")
            del self.parity[field]
            self.write_json("parity-report.json", self.parity)
            self.assert_rejected("normative approval counters")
            self.parity[field] = correct

    def test_normative_case_order_is_not_significant(self):
        self.parity["normative_cases"].reverse()
        self.write_json("parity-report.json", self.parity)
        result = self.verify()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("normative_passed=1, normative_blocked=29", result.stdout)

    def test_blocked_counter_includes_not_run_cases(self):
        for status in ("BLOCKED", "NOT_RUN"):
            with self.subTest(status=status):
                for case in self.parity["normative_cases"]:
                    if case["id"] != "PAR-08":
                        case["status"] = status
                self.write_json("parity-report.json", self.parity)
                result = self.verify()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("normative_passed=1, normative_blocked=29", result.stdout)

    def test_changed_snapshot_is_rejected_even_when_json_is_valid(self):
        self.write_json(
            "schema-snapshots/lab-v1.json", {"synthetic_snapshot": "changed"}
        )
        self.assert_rejected()

    def test_missing_and_truncated_snapshots_are_rejected(self):
        for name in SNAPSHOTS:
            path = self.root / "schema-snapshots" / name
            original = path.read_bytes()
            for mutation in ("missing", "truncated"):
                with self.subTest(name=name, mutation=mutation):
                    if mutation == "missing":
                        path.unlink()
                    else:
                        path.write_bytes(original[:1])
                    self.assert_rejected()
                    path.write_bytes(original)

    def test_missing_and_truncated_manifest_are_rejected(self):
        path = self.root / "schema-snapshots/technical-digests.json"
        for mutation in ("missing", "truncated"):
            with self.subTest(mutation=mutation):
                if mutation == "missing":
                    path.unlink()
                else:
                    path.write_text("{", encoding="utf-8")
                self.assert_rejected()
                self.write_manifest()

    def test_incomplete_duplicate_and_unsafe_manifest_paths_are_rejected(self):
        original = self.manifest["snapshots"]
        for entries in (
            [],
            original[:-1],
            [*original[:-1], original[0]],
            [*original, {"path": "../outside.json", "sha256": "0" * 64}],
            [*original[:-1], {"path": "/outside.json", "sha256": "0" * 64}],
        ):
            with self.subTest(entries=entries):
                self.manifest["snapshots"] = entries
                self.write_manifest()
                self.assert_rejected()

    def test_malformed_manifest_structure_is_rejected(self):
        for manifest in (
            None,
            [],
            {},
            {"snapshots": None},
            {"snapshots": {}},
            {"snapshots": [None] * 7},
            {"snapshots": [{"path": []}] * 7},
        ):
            with self.subTest(manifest=manifest):
                self.write_json("schema-snapshots/technical-digests.json", manifest)
                self.assert_rejected()

    def test_snapshot_symlinks_are_rejected(self):
        for relative in (
            "schema-snapshots/lab-v1.json",
            "schema-snapshots/technical-digests.json",
            "schema-snapshots",
        ):
            with self.subTest(relative=relative):
                path = self.root / relative
                original = path.with_name(path.name + ".original")
                path.rename(original)
                try:
                    path.symlink_to(
                        original.name, target_is_directory=original.is_dir()
                    )
                    self.assert_rejected()
                finally:
                    path.unlink()
                    original.rename(path)

    def test_invalid_missing_and_mismatched_digests_are_rejected(self):
        entry = self.manifest["snapshots"][0]
        for digest in (None, "", "a" * 63, "g" * 64, 42, "0" * 64):
            with self.subTest(digest=digest):
                entry["sha256"] = digest
                self.write_manifest()
                self.assert_rejected()
        del entry["sha256"]
        self.write_manifest()
        self.assert_rejected()


if __name__ == "__main__":
    unittest.main()
