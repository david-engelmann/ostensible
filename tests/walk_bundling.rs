//! The pins of PORT12, the walk bundling, the discharge of
//! debt B-2 of PORT8-6. The first pin reads the walk's
//! signature in the crate's own source, and the other two
//! drive the scan over synthetic sources the suite writes
//! itself, standing as the border proof that the bundling
//! moved no answer the walk gives.

use std::path::Path;

use ostensible::pyscan::scan_source;

#[test]
fn test_the_d5_walk_takes_one_bundled_value() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("pyscan.rs");
    let source = std::fs::read_to_string(path).expect("the scan source stands in the tree");
    let stripped: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        stripped.contains("structD5Walk"),
        "the bundle stands as a named struct"
    );
    assert!(
        stripped.contains("fnvisit(walk:D5Walk<'_,'_>)"),
        "the visit takes the one bundled value"
    );
    assert!(
        !stripped.contains("path:&str,findings:&mutVec<StaticFinding>"),
        "the path and the sink no longer travel loose beside the structs"
    );
}

#[test]
fn test_the_bundled_walk_still_finds_across_nested_scopes() {
    let source = "from unittest.mock import Mock\n\ndef make_client():\n    svc = Mock()\n    svc.fetch.return_value = 42\n    return svc\n\ndef test_fetch():\n    def run():\n        client = make_client()\n        assert client.fetch() == 42\n    run()\n";
    let findings = scan_source(source, "test_nested.py", None, None)
        .expect("the scan reads the nested source");
    assert_eq!(findings.len(), 1, "the nested assert earns its finding");
    let finding = &findings[0];
    assert_eq!(finding.kind, "mock_return_value");
    assert_eq!(finding.path, "test_nested.py");
    assert_eq!(finding.line, 11);
    assert_eq!(
        finding.detail,
        "assertion checks a call on a mock against the return value that call was assigned in a helper or fixture in the same file, 42"
    );
}

#[test]
fn test_the_bundled_walk_still_carries_fixture_bindings() {
    let source = "import pytest\nfrom unittest.mock import Mock\n\n@pytest.fixture\ndef client():\n    built = Mock()\n    built.fetch.return_value = 42\n    return built\n\ndef test_fetch(client):\n    assert client.fetch() == 42\n";
    let findings = scan_source(source, "test_fixture.py", None, None)
        .expect("the scan reads the fixture source");
    assert_eq!(
        findings.len(),
        1,
        "the fixture bound assert earns its finding"
    );
    let finding = &findings[0];
    assert_eq!(finding.kind, "mock_return_value");
    assert_eq!(finding.path, "test_fixture.py");
    assert_eq!(finding.line, 11);
    assert_eq!(
        finding.detail,
        "assertion checks a call on a mock against the return value that call was assigned in a helper or fixture in the same file, 42"
    );
}
