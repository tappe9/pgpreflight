#!/usr/bin/env python3
"""Run the local release-readiness checks in CI order."""

from __future__ import annotations

import subprocess


COMMANDS = (
    ("cargo", "+stable", "metadata", "--locked", "--format-version", "1", "--no-deps"),
    ("cargo", "+stable", "publish", "--dry-run", "--locked", "-p", "pgpreflight-core"),
    (
        "cargo", "+stable", "package",
        "--locked", "--no-verify", "-p", "pgpreflight-postgres",
        "--config", 'patch.crates-io.pgpreflight-core.path="crates/pgpreflight-core"',
    ),
    (
        "cargo", "+stable", "package",
        "--locked", "--no-verify", "-p", "pgpreflight",
        "--config", 'patch.crates-io.pgpreflight-core.path="crates/pgpreflight-core"',
        "--config", 'patch.crates-io.pgpreflight-postgres.path="crates/pgpreflight-postgres"',
    ),
    ("python3", "-m", "unittest", "discover", "-s", "scripts/tests", "-v"),
)


def main() -> None:
    for command in COMMANDS:
        subprocess.run(command, check=True)


if __name__ == "__main__":
    main()
