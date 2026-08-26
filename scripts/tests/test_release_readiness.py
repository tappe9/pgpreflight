from __future__ import annotations

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

    def test_required_ci_aggregates_release_readiness(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        self.assertIn("release-readiness:", workflow)
        self.assertIn("needs.release-readiness.result", workflow)


if __name__ == "__main__":
    unittest.main()
