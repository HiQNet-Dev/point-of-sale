"""Explicit native tests: each server is a child owned by this process, never external."""

import argparse
import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/java_source_lab.py"
spec = importlib.util.spec_from_file_location("java_source_lab", SCRIPT)
lab = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lab)
BIN = None


class NativeCleanupTests(unittest.TestCase):
    def test_failure_after_ready_stops_and_reaps(self):
        with tempfile.TemporaryDirectory(dir=lab.ROOT / "backend/target") as temp:
            work = Path(temp)
            cluster = lab.Cluster(BIN, work)
            with self.assertRaisesRegex(lab.LabError, "injected"):
                try:
                    cluster.start()
                    raise lab.LabError("injected after readiness")
                finally:
                    cluster.stop()
            self.assertIsNotNone(cluster.child.poll())
            self.assertFalse((cluster.data / "postmaster.pid").exists())
        self.assertFalse(work.exists())

    def test_failure_during_startup_stops_and_reaps(self):
        original = lab.run

        def fail_ready(*args, **kwargs):
            if kwargs.get("stage") == "ready":
                raise RuntimeError("injected during readiness")
            return original(*args, **kwargs)

        with tempfile.TemporaryDirectory(dir=lab.ROOT / "backend/target") as temp:
            work = Path(temp)
            cluster = lab.Cluster(BIN, work)
            with self.assertRaisesRegex(RuntimeError, "injected"):
                try:
                    with patch.object(lab, "run", fail_ready):
                        cluster.start()
                finally:
                    cluster.stop()
            self.assertIsNotNone(cluster.child.poll())
        self.assertFalse(work.exists())

    def test_pg_ctl_failure_still_kills_and_waits_owned_child(self):
        with tempfile.TemporaryDirectory(dir=lab.ROOT / "backend/target") as temp:
            work = Path(temp)
            cluster = lab.Cluster(BIN, work)
            try:
                cluster.start()
            finally:
                with patch.object(
                    lab, "run", side_effect=lab.LabError("injected pg_ctl failure")
                ):
                    # El fallo de pg_ctl no sustituye el resultado del trabajo: se
                    # mata y recolecta el hijo y se informa KILLED_AND_REAPED.
                    self.assertEqual(cluster.stop(), "KILLED_AND_REAPED")
            self.assertIsNotNone(cluster.child.poll())
        self.assertFalse(work.exists())

    def test_initdb_failure_never_starts_a_server(self):
        with tempfile.TemporaryDirectory(dir=lab.ROOT / "backend/target") as temp:
            work = Path(temp)
            cluster = lab.Cluster(BIN, work)
            with self.assertRaisesRegex(lab.LabError, "injected"):
                try:
                    with patch.object(
                        lab, "run", side_effect=lab.LabError("injected initdb failure")
                    ):
                        cluster.start()
                finally:
                    cluster.stop()
            self.assertIsNone(cluster.child)
        self.assertFalse(work.exists())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pg-bin", required=True, type=Path)
    args = parser.parse_args()
    BIN = lab.pg_bin(args.pg_bin)
    unittest.main(argv=[__file__], verbosity=2)
