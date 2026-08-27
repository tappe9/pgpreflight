#!/usr/bin/env python3
"""Create pgpreflight release archives and SHA-256 sidecars."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tarfile
import tempfile
import zipfile
from pathlib import Path


def archive_filename(version: str, target: str, extension: str) -> str:
    return f"pgpreflight-v{version}-{target}.{extension}"


def write_sha256(path: Path) -> Path:
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    sidecar = path.with_name(f"{path.name}.sha256")
    sidecar.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return sidecar


def package(binary: Path, version: str, target: str, output: Path) -> Path:
    extension = "zip" if target.endswith("windows-msvc") else "tar.gz"
    archive = output / archive_filename(version, target, extension)
    root_name = archive.name.removesuffix(f".{extension}")
    binary_name = "pgpreflight.exe" if extension == "zip" else "pgpreflight"
    output.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as temporary_directory:
        root = Path(temporary_directory) / root_name
        root.mkdir()
        shutil.copy2(binary, root / binary_name)
        for filename in ("README.md", "README.ja.md", "LICENSE-APACHE", "LICENSE-MIT"):
            shutil.copy2(filename, root / filename)

        if extension == "zip":
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
                for path in sorted(root.iterdir()):
                    bundle.write(path, f"{root_name}/{path.name}")
        else:
            with tarfile.open(archive, "w:gz") as bundle:
                bundle.add(root, arcname=root_name)

    write_sha256(archive)
    return archive


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, default=Path("dist"))
    arguments = parser.parse_args()
    package(arguments.binary, arguments.version, arguments.target, arguments.output)


if __name__ == "__main__":
    main()
