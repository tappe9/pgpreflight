const CI_WORKFLOW: &str = include_str!("../../../.github/workflows/ci.yml");

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn assert_workflow_contains(expected: &str, contract: &str) {
    let workflow = normalize_line_endings(CI_WORKFLOW);
    assert!(
        workflow.contains(expected),
        "CI workflow does not satisfy {contract}; missing:\n{expected}"
    );
}

#[test]
fn normalizes_windows_checkout_line_endings_before_matching_contracts() {
    assert_eq!(
        normalize_line_endings("jobs:\r\n  quality:\r\n"),
        "jobs:\n  quality:\n"
    );
}

#[test]
fn exposes_stable_job_names_and_one_aggregate_required_check() {
    for (expected, contract) in [
        ("\n  quality:\n    name: quality\n", "the quality job name"),
        ("\n  msrv:\n    name: msrv\n", "the MSRV job name"),
        (
            "\n  cross-platform:\n    name: cross-platform / non-db (${{ matrix.os }})\n",
            "the cross-platform job name",
        ),
        (
            "\n  postgresql:\n    name: postgresql / ${{ matrix.postgresql }}\n",
            "the PostgreSQL job name",
        ),
        (
            "\n  required:\n    name: required\n",
            "the aggregate required check",
        ),
    ] {
        assert_workflow_contains(expected, contract);
    }

    assert_workflow_contains(
        "    needs:\n      - quality\n      - msrv\n      - cross-platform\n      - postgresql\n",
        "the aggregate dependency list",
    );
    assert_workflow_contains(
        "    if: ${{ always() }}\n",
        "the aggregate always-run guard",
    );
}

#[test]
fn runs_non_database_builds_and_tests_on_linux_macos_and_windows() {
    assert_workflow_contains(
        "        os:\n          - ubuntu-latest\n          - macos-latest\n          - windows-latest\n",
        "the Linux/macOS/Windows matrix",
    );
    assert_workflow_contains(
        "      - run: cargo +stable build --workspace --all-targets --all-features\n",
        "the cross-platform build contract",
    );
    assert_workflow_contains(
        "      - run: cargo +stable test --workspace --all-features\n",
        "the cross-platform non-database test contract",
    );
}

#[test]
fn runs_semantic_integration_tests_against_postgresql_14_through_18() {
    assert_workflow_contains(
        "        postgresql:\n          - 14\n          - 15\n          - 16\n          - 17\n          - 18\n",
        "the PostgreSQL 14-18 matrix",
    );
    assert_workflow_contains(
        "        image: postgres:${{ matrix.postgresql }}-alpine\n",
        "the PostgreSQL service image matrix",
    );
    assert_workflow_contains(
        "      PGPREFLIGHT_TEST_POSTGRES_MAJOR: ${{ matrix.postgresql }}\n",
        "the expected PostgreSQL-major assertion",
    );
}

#[test]
fn checks_the_declared_rust_1_85_msrv_explicitly() {
    assert_workflow_contains("          toolchain: 1.85.0\n", "the Rust 1.85.0 toolchain");
    assert_workflow_contains(
        "      - run: cargo +1.85.0 check --workspace --all-targets --all-features\n",
        "the explicit MSRV workspace check",
    );
}
