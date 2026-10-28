#!/usr/bin/env python3
"""Owned PostgreSQL 17 + real Flyway source fixture. No target connection."""

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

MAX_SECONDS = 200
ROOT = Path(__file__).resolve().parents[4]
FIXTURES = Path(__file__).resolve().parents[1] / "fixtures/source"


class LabError(Exception):
    """Safe diagnostic: never include subprocess output, SQL or credentials."""


def child_env():
    env = {
        key: os.environ[key]
        for key in ("PATH", "HOME", "JAVA_HOME")
        if key in os.environ
    }
    env.update({"LC_ALL": "C", "TZ": "UTC"})
    return env


def kill_wait(child):
    # Every child owns a new process group, including Maven's/JVM's descendants.
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    child.wait(timeout=10)


def run(argv, *, stage, env=None, cwd=ROOT, timeout=MAX_SECONDS, data=None):
    child = subprocess.Popen(
        [str(arg) for arg in argv],
        cwd=cwd,
        env=env if env is not None else child_env(),
        stdin=subprocess.PIPE if data is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        output, _ = child.communicate(input=data, timeout=min(timeout, MAX_SECONDS))
        if child.returncode:
            marker = re.search(
                rb"source fixture failed at (self-test|create-source|flyway|seed|export) class=[A-Za-z]+\b",
                output,
            )
            detail = marker.group().decode() if marker else "output withheld"
            frames = re.findall(rb"source-(?:cause|frame) [A-Za-z0-9_.$:<>-]+", output)
            detail += " " + "; ".join(frame.decode() for frame in frames)
            raise LabError(
                f"{stage}: subprocess failed (exit {child.returncode}); {detail}"
            )
        return output.decode("utf-8")
    except BaseException as error:
        kill_wait(child)
        if isinstance(error, subprocess.TimeoutExpired):
            raise LabError(
                f"{stage}: subprocess timeout; owned process group killed/reaped"
            ) from None
        raise
    finally:
        if child.stdout is not None:
            child.stdout.close()
        if child.stdin is not None:
            child.stdin.close()


def create_output(path):
    path = Path(path)
    if not path.name or ".." in path.parts or ":" in str(path):
        raise LabError("--output must be a new directory, without '..' or a URL")
    try:
        parent = path.parent.resolve(strict=True)
        result = parent / path.name
        result.mkdir(mode=0o700)
        return result
    except OSError:
        raise LabError(
            "--output must not exist and its parent must already exist"
        ) from None


def private_json(path, value):
    data = (
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        + "\n"
    ).encode()
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(data)
    return hashlib.sha256(data).hexdigest()


def pg_bin(path):
    path = Path(path).resolve(strict=True)
    for name in ("postgres", "initdb", "pg_ctl", "pg_isready"):
        version = run([path / name, "--version"], stage=f"version-{name}", timeout=10)
        if not re.search(r"\b17\.\d+\b", version):
            raise LabError("all PostgreSQL binaries must be version 17.x")
    return path


class Cluster:
    def __init__(self, bin_path, work):
        self.bin = bin_path
        self.work = work
        self.data = work / "data"
        self.child = None
        self.password = secrets.token_urlsafe(32)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            self.port = reservation.getsockname()[1]
        self.env = child_env()
        self.env["LAB_PASSWORD"] = self.password
        self.env["LAB_PORT"] = str(self.port)

    def start(self):
        run(
            [
                self.bin / "initdb",
                "-D",
                self.data,
                "-U",
                "lab_admin",
                "--encoding=UTF8",
                "--locale=C",
                "--auth-local=scram-sha-256",
                "--auth-host=scram-sha-256",
                "--pwfile=/dev/stdin",
            ],
            stage="initdb",
            env=self.env,
            data=(self.password + "\n").encode(),
        )
        self.child = subprocess.Popen(
            [
                str(self.bin / "postgres"),
                "-D",
                str(self.data),
                "-h",
                "127.0.0.1",
                "-p",
                str(self.port),
                "-c",
                "unix_socket_directories=",
                "-c",
                "password_encryption=scram-sha-256",
                "-c",
                "logging_collector=off",
                "-c",
                "log_statement=none",
                "-c",
                "log_min_messages=panic",
                "-c",
                "log_min_error_statement=panic",
            ],
            env=child_env(),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if self.child.poll() is not None:
                raise LabError("owned PostgreSQL exited during startup")
            try:
                run(
                    [
                        self.bin / "pg_isready",
                        "-h",
                        "127.0.0.1",
                        "-p",
                        str(self.port),
                        "-U",
                        "lab_admin",
                        "-d",
                        "postgres",
                        "-t",
                        "2",
                    ],
                    stage="ready",
                    timeout=5,
                )
                return
            except LabError:
                time.sleep(0.1)
        raise LabError("owned PostgreSQL readiness timeout")

    def stop(self):
        if self.child is None:
            return "NO_SERVER_STARTED"
        graceful = True
        try:
            if self.child.poll() is None:
                try:
                    run(
                        [
                            self.bin / "pg_ctl",
                            "-D",
                            self.data,
                            "-m",
                            "fast",
                            "-w",
                            "-t",
                            "15",
                            "stop",
                        ],
                        stage="pg_ctl-stop",
                        timeout=20,
                    )
                    self.child.wait(timeout=10)
                except (LabError, OSError, subprocess.TimeoutExpired):
                    # Startup may fail before postmaster.pid exists; still reap our child
                    # without replacing the original failure with a pg_ctl error.
                    graceful = False
        finally:
            kill_wait(self.child)
        return (
            "STOPPED_AND_REAPED"
            if graceful and not (self.data / "postmaster.pid").exists()
            else "KILLED_AND_REAPED"
        )


def migration_snapshot(work):
    source = ROOT / "backend/src/main/resources/db/migration"
    files = sorted(
        source.glob("V*__*.sql"), key=lambda p: int(p.name.split("__")[0][1:])
    )
    if [int(p.name.split("__")[0][1:]) for p in files] != list(range(1, 26)):
        raise LabError("source must contain exactly Flyway V1 through V25")
    dest = work / "migrations"
    dest.mkdir(mode=0o700)
    hashes = {}
    for file in files:
        data = file.read_bytes()
        (dest / file.name).write_bytes(data)
        hashes[file.name] = hashlib.sha256(data).hexdigest()
    return source, dest, hashes


def execute(bin_path, output):
    target = ROOT / "backend/target"
    target.mkdir(exist_ok=True)
    report = {
        "version": 1,
        "kind": "source-report",
        "status": "FAILED",
        "coverage": "synthetic_source_observation",
        "target_import": "BLOCKED_CONFLICT_FREEZE",
        "normative_status": "UNCHANGED_BLOCKED",
        "observations": [],
        "blocked_cases": [],
        "actions": {"refresh_sessions": "INVALIDATE", "app_metadata": "REFERENCE_ONLY"},
    }
    try:
        with tempfile.TemporaryDirectory(prefix="java-source-lab-", dir=target) as temp:
            work = Path(temp)
            work.chmod(0o700)
            source, migrations, hashes = migration_snapshot(work)
            cpfile = work / "classpath.txt"
            run(
                [
                    ROOT / "backend/mvnw",
                    "-B",
                    "-q",
                    "dependency:build-classpath",
                    "-DincludeScope=runtime",
                    f"-Dmdep.outputFile={cpfile}",
                ],
                stage="maven-classpath",
                cwd=ROOT / "backend",
            )
            classpath = cpfile.read_text().strip()
            if not classpath:
                raise LabError("empty Maven runtime classpath")
            digests = []
            for index in (1, 2):
                owned = work / f"install-{index}"
                owned.mkdir(mode=0o700)
                cluster = Cluster(bin_path, owned)
                observation = {
                    "id": f"SOURCE-INSTALL-{index}",
                    "status": "FAILED",
                    "source_counts": None,
                    "cluster_cleanup": "NOT_STARTED",
                }
                report["observations"].append(observation)
                try:
                    cluster.start()
                    env = dict(cluster.env)
                    env.update(
                        {
                            "LAB_MIGRATIONS": str(migrations),
                            "LAB_SEED": str(FIXTURES / "seed.sql"),
                            "LAB_WORK": str(owned),
                        }
                    )
                    run(
                        ["java", "-cp", classpath, FIXTURES / "SourceFixture.java"],
                        stage=f"flyway-seed-export-{index}",
                        env=env,
                    )
                    bundle = json.loads((owned / "bundle.json").read_text())
                    repeat = json.loads((owned / "repeat.json").read_text())
                    if bundle != repeat:
                        raise LabError("same-snapshot repeated exporter differs")
                    digest = private_json(output / f"bundle-{index}.json", bundle)
                    digests.append(digest)
                    history = json.loads((owned / "history.json").read_text())
                    private_json(output / f"flyway-history-{index}.json", history)
                    counts = {
                        table["name"]: table["source_count"]
                        for table in bundle["tables"]
                    }
                    missing = [name for name, count in counts.items() if count == 0]
                    observation.update(
                        {
                            "status": "PASSED"
                            if not missing and len(counts) == 37
                            else "BLOCKED",
                            "source_counts": counts,
                            "populated_tables": sum(n > 0 for n in counts.values()),
                            "table_count": len(counts),
                            "functional_bundle_sha256": digest,
                            "same_snapshot_repeat": "PASSED",
                            "flyway_versions": len(history),
                        }
                    )
                    if missing or len(counts) != 37:
                        report["blocked_cases"].append(
                            {"id": "SOURCE-37-COVERAGE", "tables_without_rows": missing}
                        )
                finally:
                    try:
                        observation["cluster_cleanup"] = cluster.stop()
                    except BaseException:
                        observation["cluster_cleanup"] = (
                            "STOP_FAILED_KILL_WAIT_ATTEMPTED"
                        )
                        observation["status"] = "FAILED"
                        raise

            if hashes != {
                name: hashlib.sha256((source / name).read_bytes()).hexdigest()
                for name in hashes
            }:
                raise LabError("source migrations changed during execution")
            equal = digests[0] == digests[1]
            report["observations"].append(
                {
                    "id": "SOURCE-RECONSTRUCTION",
                    "status": "PASSED" if equal else "FAILED",
                    "independent_installs": 2,
                    "equal_functional_digests": equal,
                }
            )
            if not equal:
                raise LabError("independent source reconstructions differ")
            private_json(
                output / "manifest.json",
                {
                    "version": 1,
                    "source_migrations_sha256": hashes,
                    "seed_sha256": hashlib.sha256(
                        (FIXTURES / "seed.sql").read_bytes()
                    ).hexdigest(),
                    "runner_sha256": hashlib.sha256(
                        (FIXTURES / "SourceFixture.java").read_bytes()
                    ).hexdigest(),
                    "bundle_encoding": "canonical-json-v1",
                    "private_synthetic_credentials": True,
                    "postgres_version": run(
                        [bin_path / "postgres", "--version"],
                        stage="version",
                        timeout=10,
                    ).strip(),
                    "excluded": [
                        "flyway_schema_history technical timestamps",
                        "refresh_sessions rows",
                    ],
                },
            )
            report["status"] = "BLOCKED" if report["blocked_cases"] else "PASSED"
    except Exception as error:
        report["failure"] = (
            str(error) if isinstance(error, LabError) else type(error).__name__
        )
        report["blocked_cases"].append({"id": "SOURCE-INCOMPLETE", "status": "BLOCKED"})
        raise
    finally:
        # PAR-08 mide el exportador determinista sobre dos instalaciones limpias;
        # no requiere importar al target. El import sigue bloqueado por el freeze.
        observations = report.get("observations", [])
        installs = [
            item
            for item in observations
            if item.get("id", "").startswith("SOURCE-INSTALL-")
        ]
        reconstruction = next(
            (
                item
                for item in observations
                if item.get("id") == "SOURCE-RECONSTRUCTION"
            ),
            None,
        )
        par08_ok = (
            report.get("status") == "PASSED"
            and len(installs) == 2
            and all(
                item.get("status") == "PASSED"
                and item.get("table_count") == 37
                and item.get("populated_tables") == 37
                for item in installs
            )
            and reconstruction is not None
            and reconstruction.get("status") == "PASSED"
            and reconstruction.get("equal_functional_digests") is True
        )
        report["observations"].append(
            {
                "id": "PAR-08",
                "status": "PASSED" if par08_ok else "BLOCKED",
                "coverage": "source_exporter_determinism",
                "equal_functional_digests": par08_ok,
                "limitation": "No importa ni compara el target; TARGET-IMPORT sigue BLOCKED",
            }
        )
        report["blocked_cases"].append(
            {
                "id": "TARGET-IMPORT",
                "status": "BLOCKED",
                "reason": "physical contract freeze; no target contacted",
            }
        )
        private_json(output / "source-report.json", report)
    return 0 if report["status"] == "PASSED" else 2


def interrupted(signum, frame):
    raise LabError("interrupted; cleaning owned children")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pg-bin",
        required=True,
        type=Path,
        help="directory containing PostgreSQL 17 binaries",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="NEW directory with existing parent (private evidence)",
    )
    args = parser.parse_args(argv)
    previous = {
        sig: signal.signal(sig, interrupted) for sig in (signal.SIGINT, signal.SIGTERM)
    }
    old_umask = os.umask(0o077)
    try:
        # Validate output before any subprocess and never accept a supplied database URL.
        output = create_output(args.output)
        result = execute(pg_bin(args.pg_bin), output)
        print("Source-only observations written; target/parity gates unchanged.")
        return result
    except (LabError, OSError) as error:
        if isinstance(error, LabError):
            print(str(error), file=sys.stderr)
        print(
            "source lab failed; no subprocess payload disclosed; inspect private source-report.json if present",
            file=sys.stderr,
        )
        return 1
    finally:
        os.umask(old_umask)
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == "__main__":
    sys.exit(main())
