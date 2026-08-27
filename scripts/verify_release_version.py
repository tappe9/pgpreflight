#!/usr/bin/env python3
"""Require a release tag/input to match the Cargo workspace version."""

from __future__ import annotations

import argparse
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def verify_version(release_version: str) -> None:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    expected = workspace["workspace"]["package"]["version"]
    actual = release_version.removeprefix("v")
    if actual != expected:
        raise ValueError(f"release version {actual!r} does not match workspace version {expected!r}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    arguments = parser.parse_args()
    try:
        verify_version(arguments.version)
    except ValueError as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
