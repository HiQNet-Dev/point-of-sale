"""CI regression tests; requires Python 3, Ruby/Psych, git and jq (runner defaults)."""

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DETECTOR = ROOT / ".github/scripts/ci-changes.sh"
WORKFLOW = json.loads(
    subprocess.check_output(
        [
            "ruby",
            "-rjson",
            "-ryaml",
            "-e",
            "puts JSON.generate(YAML.load_file(ARGV.fetch(0)))",
            str(ROOT / ".github/workflows/ci.yml"),
        ],
        text=True,
    )
)


class ChangesTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="ci-changes-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.git("init", "--quiet")
        # Synthetic history exists only in this temporary repo, never the checkout.
        self.git("config", "user.name", "CI tests")
        self.git("config", "user.email", "ci-tests@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.hooksPath", "/dev/null")
        self.base = self.commit({"README.md": "base"})

    def git(self, *args):
        return subprocess.check_output(
            ["git", "--no-pager", *args],
            cwd=self.root,
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()

    def commit(self, files):
        for path, contents in files.items():
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(contents)
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "test fixture")
        return self.git("rev-parse", "HEAD")

    def detect(self, files=None, event="pull_request", base=None):
        if files:
            self.commit(files)
        if base is None:
            base = self.base
        output_file = self.root / ".git/outputs"
        output_file.write_text("")
        env = dict(
            os.environ,
            EVENT_NAME=event,
            BASE_SHA=base,
            BEFORE_SHA=base,
            GITHUB_OUTPUT=str(output_file),
        )
        subprocess.run(
            ["bash", str(DETECTOR)],
            cwd=self.root,
            env=env,
            check=True,
            capture_output=True,
            text=True,
        )
        return dict(line.split("=", 1) for line in output_file.read_text().splitlines())

    def assert_runs(self, flags, *required):
        for area in ("backend", "rust", "frontend", "compose"):
            self.assertEqual(str(area in required).lower(), flags[f"{area}_run"], area)

    def test_rust_workspace_paths_only_require_rust(self):
        for path in (
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "crates/pos-core/src/lib.rs",
            "crates/pos-storage-sqlite/src/lib.rs",
            "apps/pos-local/src/main.rs",
            "tools/migration-lab/src/main.rs",
            ".cargo/config.toml",
        ):
            with self.subTest(path=path):
                flags = self.detect({f"backend-rust/{path}": "fixture"})
                self.assertEqual("true", flags["rust"])
                self.assertEqual("false", flags["full"])
                self.assert_runs(flags, "rust")

    def test_docs_only_requires_no_executable_jobs(self):
        flags = self.detect(
            {"docs/05-development/ci.md": "docs", "backend-rust/README.md": "docs"}
        )
        self.assertEqual("false", flags["rust"])
        self.assert_runs(flags)

    def test_legacy_areas_keep_their_own_jobs(self):
        for path, area in {
            "backend/pom.xml": "backend",
            "frontend/package.json": "frontend",
            "docker-compose.yml": "compose",
            "deploy/example.conf": "compose",
        }.items():
            with self.subTest(path=path):
                self.base = self.git("rev-parse", "HEAD")
                self.assert_runs(self.detect({path: "fixture"}), area)

    def test_mixed_java_and_rust_change(self):
        self.assert_runs(
            self.detect(
                {"backend/pom.xml": "fixture", "backend-rust/Cargo.lock": "fixture"}
            ),
            "backend",
            "rust",
        )

    def test_ci_changes_require_all_jobs(self):
        flags = self.detect({".github/workflows/ci.yml": "fixture"})
        self.assertEqual("true", flags["ci"])
        self.assert_runs(flags, "backend", "rust", "frontend", "compose")

    def test_unknown_path_falls_back_to_full(self):
        flags = self.detect({"new-component/source.txt": "fixture"})
        self.assertEqual("true", flags["full"])
        self.assert_runs(flags, "backend", "rust", "frontend", "compose")

    def test_manual_and_schedule_force_full_even_without_diff(self):
        for event in ("workflow_dispatch", "schedule"):
            with self.subTest(event=event):
                flags = self.detect(event=event)
                self.assertEqual("true", flags["full"])
                self.assert_runs(flags, "backend", "rust", "frontend", "compose")

    def test_missing_zero_or_invalid_base_forces_full(self):
        for base in ("", "0" * 40, "f" * 40):
            with self.subTest(base=base):
                flags = self.detect(base=base)
                self.assertEqual("true", flags["full"])
                self.assert_runs(flags, "backend", "rust", "frontend", "compose")

    def test_push_uses_before_sha(self):
        self.assert_runs(
            self.detect({"backend-rust/Cargo.lock": "fixture"}, event="push"), "rust"
        )

    def test_empty_diff_does_not_require_jobs(self):
        self.assert_runs(self.detect())

    def test_deleted_rust_file_still_requires_rust(self):
        self.base = self.commit({"backend-rust/crates/pos-core/src/lib.rs": "fixture"})
        self.git("rm", "backend-rust/crates/pos-core/src/lib.rs")
        self.git("commit", "--quiet", "-m", "delete fixture")
        self.assert_runs(self.detect(), "rust")


class RustWorkflowTest(unittest.TestCase):
    jobs = WORKFLOW["jobs"]

    def gate_result(
        self, rust_run, rust_result, changes_result="success", backend_result="skipped"
    ):
        needs = {
            "changes": {"result": changes_result, "outputs": {"rust_run": rust_run}},
            "backend": {"result": backend_result},
            "rust": {"result": rust_result},
            "frontend": {"result": "skipped"},
            "compose": {"result": "skipped"},
            "e2e": {"result": "skipped"},
        }
        script = next(
            step["run"]
            for step in self.jobs["ci-gate"]["steps"]
            if step.get("name") == "Verify required jobs"
        )
        return (
            subprocess.run(
                ["bash", "-euo", "pipefail", "-c", script],
                env=dict(os.environ, NEEDS=json.dumps(needs)),
                capture_output=True,
                check=False,
            ).returncode
            == 0
        )

    def test_required_rust_accepts_only_success(self):
        for result in ("success", "skipped", "failure", "cancelled"):
            with self.subTest(result=result):
                self.assertEqual(result == "success", self.gate_result("true", result))

    def test_non_required_rust_can_be_skipped_but_not_failed(self):
        for result in ("success", "skipped", "failure", "cancelled"):
            with self.subTest(result=result):
                self.assertEqual(
                    result in ("success", "skipped"), self.gate_result("false", result)
                )

    def test_missing_or_invalid_detection_cannot_approve_gate(self):
        for flag in (None, "", "unexpected"):
            with self.subTest(flag=flag):
                self.assertFalse(self.gate_result(flag, "skipped"))
        for result in ("failure", "cancelled", "skipped"):
            with self.subTest(result=result):
                self.assertFalse(
                    self.gate_result("true", "success", changes_result=result)
                )

    def test_legacy_failure_still_fails_gate(self):
        for result in ("failure", "cancelled"):
            with self.subTest(result=result):
                self.assertFalse(
                    self.gate_result("true", "success", backend_result=result)
                )

    def test_rust_job_contract_and_partial_evidence(self):
        rust = self.jobs["rust"]
        self.assertEqual("needs.changes.outputs.rust_run == 'true'", rust["if"])
        self.assertEqual("backend-rust", rust["defaults"]["run"]["working-directory"])
        steps = rust["steps"]
        commands = "\n".join(step.get("run", "") for step in steps)
        for command in (
            "cargo fmt --all -- --check",
            "cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
            "cargo test --locked --workspace --all-features",
            "cargo build --locked --workspace --all-features",
            "cargo run --locked -p migration-lab -- --output target/migration-evidence --postgres-bin target/pg17-build/install/bin --java-source",
            "cargo install cargo-audit --version 0.22.2 --locked",
            "cargo audit --file Cargo.lock",
            "sh tools/migration-lab/scripts/build_postgres.sh",
            "python3 tools/migration-lab/scripts/verify_evidence.py --evidence target/migration-evidence --expect-exit 0",
            "python3 -B tools/migration-lab/scripts/test_verify_evidence.py",
        ):
            self.assertIn(command, commands)
        self.assertIn("rustup toolchain install 1.99.1", commands)
        self.assertTrue(
            any(step.get("uses") == "actions/setup-java@v4" for step in steps)
        )
        self.assertIn("--require-complete", commands)
        self.assertNotIn("--ignore", commands)
        self.assertFalse(any(step.get("continue-on-error") for step in steps))
        upload = next(
            step for step in steps if step.get("uses") == "actions/upload-artifact@v4"
        )
        self.assertEqual("always()", upload["if"])
        self.assertIn("backend-rust/target/migration-evidence", upload["with"]["path"])
        self.assertIn("rust", self.jobs["ci-gate"]["needs"])
        self.assertEqual(
            "${{ steps.detect.outputs.rust_run }}",
            self.jobs["changes"]["outputs"]["rust_run"],
        )

    def test_normative_gate_is_last_blocking_check_and_uploads_both_outputs(self):
        rust = self.jobs["rust"]
        self.assertEqual("Rust (technical checks + mandatory DDL13 gate)", rust["name"])
        self.assertNotIn("continue-on-error", rust)
        self.assertTrue(all("continue-on-error" not in step for step in rust["steps"]))
        steps = rust["steps"]
        audit = next(
            step
            for step in steps
            if step.get("name") == "Audit committed Cargo lockfile"
        )
        gate = next(
            step
            for step in steps
            if step.get("name")
            == "Require complete normative migration evidence (blocks merge)"
        )
        upload = next(
            step for step in steps if step.get("uses") == "actions/upload-artifact@v4"
        )
        self.assertLess(steps.index(audit), steps.index(gate))
        self.assertLess(steps.index(gate), steps.index(upload))
        self.assertEqual(
            "cargo run --locked -p migration-lab -- --output target/migration-gate-evidence --postgres-bin target/pg17-build/install/bin --java-source --require-complete",
            gate["run"],
        )
        self.assertEqual("always()", gate["if"])
        verify = next(
            step
            for step in rust["steps"]
            if step.get("name") == "Verify blocked gate evidence"
        )
        self.assertEqual("always()", verify["if"])
        self.assertIn("--expect-exit 2", verify["run"])
        self.assertEqual("actions/upload-artifact@v4", upload["uses"])
        self.assertEqual("always()", upload["if"])
        self.assertEqual(
            "rust-migration-technical-and-normative-evidence", upload["with"]["name"]
        )
        self.assertEqual(
            {
                "backend-rust/target/migration-evidence",
                "backend-rust/target/migration-gate-evidence",
                "backend-rust/target/cargo-audit.json",
            },
            set(upload["with"]["path"].splitlines()),
        )

    def test_normative_exit_two_propagates_and_ci_gate_blocks_failure(self):
        script = next(
            step["run"]
            for step in self.jobs["rust"]["steps"]
            if "--require-complete" in step.get("run", "")
        )
        # Simulate the process boundary, not a successful normative lab result.
        result = subprocess.run(
            ["bash", "-euo", "pipefail", "-c", "cargo() { return 2; }\n" + script],
            capture_output=True,
            check=False,
        )
        self.assertEqual(2, result.returncode)
        self.assertFalse(self.gate_result("true", "failure"))

    def test_security_scans_are_pinned_and_blocking(self):
        rust = self.jobs["rust"]
        self.assertFalse(rust.get("continue-on-error"))
        steps = {step.get("name"): step for step in rust["steps"]}
        install = steps["Install verified secret scanner"]["run"]
        self.assertIn("/v8.24.3/gitleaks_8.24.3_linux_x64.tar.gz", install)
        self.assertIn(
            "9991e0b2903da4c8f6122b5c3186448b927a5da4deef1fe45271c3793f4ee29c", install
        )
        self.assertIn("sha256sum --check --strict", install)
        self.assertIn(
            "dir . --redact --no-banner", steps["Scan Rust sources for secrets"]["run"]
        )
        self.assertIn(
            "git ls-files --error-unmatch Cargo.lock",
            steps["Verify checked-out target and committed lockfile"]["run"],
        )
        self.assertEqual(
            "cargo audit --file Cargo.lock --json > target/cargo-audit.json",
            steps["Audit committed Cargo lockfile"]["run"],
        )
        always_run = {
            step.get("name")
            for step in rust["steps"]
            if "run" in step and step.get("if") == "always()"
        }
        self.assertEqual(
            {
                "Require complete normative migration evidence (blocks merge)",
                "Verify blocked gate evidence",
            },
            always_run,
        )

    def test_legacy_execution_policy_and_commands(self):
        for job in ("backend", "frontend", "compose"):
            self.assertEqual(
                f"needs.changes.outputs.{job}_run == 'true'", self.jobs[job]["if"]
            )
        self.assertIn(
            "./mvnw -B -ntp clean verify",
            [step.get("run") for step in self.jobs["backend"]["steps"]],
        )
        frontend_runs = [step.get("run") for step in self.jobs["frontend"]["steps"]]
        for command in (
            "pnpm install --frozen-lockfile",
            "pnpm lint",
            "pnpm typecheck",
            "pnpm test",
            "pnpm build",
        ):
            self.assertIn(command, frontend_runs)
        self.assertIn(
            "docker compose config",
            [step.get("run") for step in self.jobs["compose"]["steps"]],
        )
        self.assertEqual(
            "github.event_name == 'workflow_dispatch' || "
            "(github.event_name == 'push' && github.ref == 'refs/heads/main')",
            self.jobs["e2e"]["if"],
        )

    def test_detector_tests_are_run_in_ci(self):
        self.assertTrue(
            any(
                step.get("run") == "python3 .github/scripts/test_ci_changes.py"
                for step in self.jobs["changes"]["steps"]
            )
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
