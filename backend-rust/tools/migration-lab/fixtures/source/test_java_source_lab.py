"""Small stdlib tests; real JDBC assertions also run inside the source lab."""

import importlib.util
import json
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/java_source_lab.py"
spec = importlib.util.spec_from_file_location("java_source_lab", SCRIPT)
lab = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lab)


class SourceLabTests(unittest.TestCase):
    def test_arguments_reject_missing_options_and_external_url(self):
        for args in (
            [],
            ["--pg-bin", "/tmp", "--output", "jdbc:postgresql://remote/db"],
        ):
            result = subprocess.run(
                [sys.executable, str(SCRIPT), *args], capture_output=True, timeout=10
            )
            self.assertNotEqual(result.returncode, 0)

    def test_output_is_exclusive_private_and_requires_existing_parent(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "new"
            self.assertEqual(lab.create_output(path), path)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o700)
            sentinel = path / "keep"
            sentinel.write_text("original")
            with self.assertRaises(lab.LabError):
                lab.create_output(path)
            with self.assertRaises(lab.LabError):
                lab.create_output(Path(tmp) / "absent/child")
            with self.assertRaises(lab.LabError):
                lab.create_output(Path(tmp) / "new/../escape")
            self.assertEqual(sentinel.read_text(), "original")
            lab.private_json(
                path / "data.json", {"null": None, "decimal": "900719925474.1234"}
            )
            self.assertEqual(stat.S_IMODE((path / "data.json").stat().st_mode), 0o600)
            self.assertEqual(
                json.loads((path / "data.json").read_text())["decimal"],
                "900719925474.1234",
            )
            with self.assertRaises(FileExistsError):
                lab.private_json(path / "data.json", {})

    def test_subprocess_timeout_and_error_do_not_disclose_payload(self):
        with self.assertRaises(lab.LabError) as caught:
            lab.run(
                [sys.executable, "-c", "import time; time.sleep(10)"],
                stage="timeout-test",
                timeout=0.05,
            )
        self.assertIn("timeout-test", str(caught.exception))
        with self.assertRaises(lab.LabError) as caught:
            lab.run(
                [
                    sys.executable,
                    "-c",
                    "import sys; print('private-payload'); sys.exit(1)",
                ],
                stage="error-test",
            )
        self.assertNotIn("private-payload", str(caught.exception))

    def test_environment_does_not_inherit_database_or_jvm_injection(self):
        with patch.dict(
            "os.environ",
            {
                "PGHOST": "production",
                "JAVA_TOOL_OPTIONS": "injected",
                "LAB_PASSWORD": "external",
            },
        ):
            env = lab.child_env()
        self.assertNotIn("PGHOST", env)
        self.assertNotIn("JAVA_TOOL_OPTIONS", env)
        self.assertNotIn("LAB_PASSWORD", env)

    def test_source_runner_uses_exact_decimals_and_not_target(self):
        source = SCRIPT.parents[1] / "fixtures/source/SourceFixture.java"
        text = source.read_text()
        self.assertIn("getBigDecimal", text)
        self.assertIn("toPlainString()", text)
        self.assertNotIn("getDouble", text)
        self.assertNotIn("getFloat", text)
        self.assertIn("TRANSACTION_REPEATABLE_READ", text)
        self.assertIn('"INVALIDATE"', text)
        self.assertNotIn("sqlx", SCRIPT.read_text().lower())


if __name__ == "__main__":
    unittest.main()
