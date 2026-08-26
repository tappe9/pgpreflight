from __future__ import annotations

import hashlib
import importlib.util
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATES = (
    "pgpreflight-core",
    "pgpreflight-postgres",
    "pgpreflight",
)
REQUIRED_TARGETS = (
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
)


class ReleaseReadinessContractTests(unittest.TestCase):
    def test_release_assets_exist(self) -> None:
        required_paths = [
            ROOT / "Cargo.lock",
            ROOT / "CHANGELOG.md",
            ROOT / ".github" / "dependabot.yml",
            ROOT / ".github" / "workflows" / "release.yml",
            ROOT / "scripts" / "package_release.py",
            ROOT / "scripts" / "verify_release_readiness.py",
        ]
        for crate in CRATES:
            crate_root = ROOT / "crates" / crate
            required_paths.extend(
                [
                    crate_root / "README.md",
                    crate_root / "LICENSE-APACHE",
                    crate_root / "LICENSE-MIT",
                ]
            )

        missing = [str(path.relative_to(ROOT)) for path in required_paths if not path.is_file()]
        self.assertEqual(missing, [], f"missing release assets: {missing}")

    def test_each_crate_declares_crates_io_metadata(self) -> None:
        for crate in CRATES:
            manifest_path = ROOT / "crates" / crate / "Cargo.toml"
            manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
            package = manifest["package"]

            for key in (
                "description",
                "homepage",
                "documentation",
                "readme",
                "keywords",
                "categories",
                "publish",
            ):
                self.assertIn(key, package, f"{crate}: package.{key} is required")

            self.assertEqual(package["readme"], "README.md")
            self.assertEqual(package["publish"], ["crates-io"])
            self.assertGreaterEqual(len(package["keywords"]), 1)
            self.assertGreaterEqual(len(package["categories"]), 1)

    def test_release_workflow_covers_native_targets_and_checksums(self) -> None:
        workflow_path = ROOT / ".github" / "workflows" / "release.yml"
        self.assertTrue(workflow_path.is_file(), ".github/workflows/release.yml is required")
        workflow = workflow_path.read_text(encoding="utf-8")

        for target in REQUIRED_TARGETS:
            self.assertIn(target, workflow)
        self.assertIn("workflow_dispatch", workflow)
        self.assertIn("tags:", workflow)
        self.assertIn("SHA256SUMS", workflow)
        self.assertIn("gh release create", workflow)

    def test_package_script_uses_stable_names_and_sha256_sidecars(self) -> None:
        script_path = ROOT / "scripts" / "package_release.py"
        self.assertTrue(script_path.is_file(), "scripts/package_release.py is required")
        spec = importlib.util.spec_from_file_location("package_release", script_path)
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        self.assertEqual(
            module.archive_filename("0.1.0", "x86_64-unknown-linux-gnu", "tar.gz"),
            "pgpreflight-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
        )
        self.assertEqual(
            module.archive_filename("0.1.0", "x86_64-pc-windows-msvc", "zip"),
            "pgpreflight-v0.1.0-x86_64-pc-windows-msvc.zip",
        )

        with tempfile.TemporaryDirectory() as temporary_directory:
            archive = Path(temporary_directory) / "artifact.tar.gz"
            archive.write_bytes(b"pgpreflight release fixture")
            sidecar = module.write_sha256(archive)
            expected = hashlib.sha256(archive.read_bytes()).hexdigest()
            self.assertEqual(sidecar.name, "artifact.tar.gz.sha256")
            self.assertEqual(sidecar.read_text(encoding="utf-8"), f"{expected}  {archive.name}\n")

    def test_public_docs_describe_the_implemented_release_candidate(self) -> None:
        stale_markers = {
            "README.md": (
                "Safe Mode planning, plan normalization, diagnostic evaluation, and the end-user `check` CLI are still",
                "rule engine itself is still planned work",
            ),
            "README.ja.md": (
                "PostgreSQL Safe Mode planning、plan normalization、diagnostic評価、利用者向け `check` CLI は今後",
                "rule engine本体は未実装",
            ),
            "ARCHITECTURE.md": ("implementation in progress", "future PostgreSQL 14–18 integration matrix"),
            "docs/REQUIREMENTS.md": ("implementation in progress", "remain future v0.1 slices"),
            "docs/API-DESIGN.md": ("remains planned",),
            "docs/COMPATIBILITY.md": ("release packaging remains pending",),
        }

        for relative_path, markers in stale_markers.items():
            content = (ROOT / relative_path).read_text(encoding="utf-8")
            for marker in markers:
                self.assertNotIn(marker, content, f"{relative_path} still contains: {marker}")

        roadmap = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
        self.assertIn("[x] **Issue #11", roadmap)
        self.assertIn("Status: **release-ready on `main`; not released**", roadmap)

    def test_required_ci_aggregates_ordered_publish_dry_run(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        self.assertIn("release-readiness:", workflow)
        self.assertIn("needs.release-readiness.result", workflow)

        metadata_index = workflow.index("cargo +stable metadata --locked")
        publish_index = workflow.index("cargo +stable publish --dry-run --locked")
        core_index = workflow.index("-p pgpreflight-core", publish_index)
        postgres_index = workflow.index("-p pgpreflight-postgres", publish_index)
        cli_index = workflow.index("-p pgpreflight", postgres_index + 1)
        self.assertLess(metadata_index, publish_index)
        self.assertLess(core_index, postgres_index)
        self.assertLess(postgres_index, cli_index)


if __name__ == "__main__":
    unittest.main()
