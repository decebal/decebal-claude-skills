use super::{is_test_file, references_in, Edge, Layer, Model};

/// Every scan in this file looks for the same forbidden edge: something under
/// `application/` reaching up into `presentation`.
fn refs(text: &str) -> Vec<super::Hit> {
    references_in("src/application/a.rs", text, "crate::", "presentation")
}
use std::collections::BTreeMap;

fn layer(name: &str) -> Layer {
    Layer {
        name: name.into(),
        dir: format!("src/{name}/"),
    }
}

fn model() -> Model {
    Model {
        order: vec![
            layer("presentation"),
            layer("application"),
            layer("infrastructure"),
        ],
        pure: vec![layer("domain")],
        path_prefix: "crate::".into(),
        ceilings: BTreeMap::new(),
        facades: BTreeMap::new(),
    }
}

fn has(edges: &[Edge], from: &str, to: &str) -> bool {
    edges.iter().any(|e| e.from.name == from && e.to.name == to)
}

#[test]
fn a_layer_may_not_import_one_before_it() {
    let edges = model().forbidden_edges();
    assert!(has(&edges, "application", "presentation"));
    assert!(has(&edges, "infrastructure", "application"));
    assert!(has(&edges, "infrastructure", "presentation"));
}

#[test]
fn importing_downward_is_not_an_edge() {
    let edges = model().forbidden_edges();
    assert!(!has(&edges, "presentation", "application"));
    assert!(!has(&edges, "application", "infrastructure"));
}

#[test]
fn a_pure_layer_may_import_no_other_layer() {
    let edges = model().forbidden_edges();
    assert!(has(&edges, "domain", "presentation"));
    assert!(has(&edges, "domain", "application"));
    assert!(has(&edges, "domain", "infrastructure"));
}

#[test]
fn a_layer_never_forbids_importing_itself() {
    assert!(!model()
        .forbidden_edges()
        .iter()
        .any(|e| e.from.name == e.to.name));
}

#[test]
fn a_file_is_owned_by_the_layer_with_the_longest_matching_dir() {
    let mut m = model();
    m.order[1].dir = "src/".into();
    let owner = m.owning_layer("src/domain/a.rs").unwrap();
    assert_eq!(owner.name, "domain");
}

#[test]
fn a_file_under_only_one_layer_is_owned_by_it() {
    let m = model();
    assert_eq!(
        m.owning_layer("src/application/a.rs").unwrap().name,
        "application"
    );
}

#[test]
fn a_file_under_no_declared_layer_is_owned_by_none() {
    let m = model();
    assert!(m.owning_layer("tooling/x.rs").is_none());
}

#[test]
fn owning_layer_reaches_pure_layers_too() {
    let m = model();
    assert_eq!(m.owning_layer("src/domain/a.rs").unwrap().name, "domain");
}

#[test]
fn a_ceiling_defaults_to_zero_and_is_keyed_by_edge() {
    let mut m = model();
    m.ceilings.insert("application_to_presentation".into(), 7);
    let edge = Edge {
        from: layer("application"),
        to: layer("presentation"),
    };
    assert_eq!(m.ceiling_for(&edge), 7);
    let other = Edge {
        from: layer("infrastructure"),
        to: layer("presentation"),
    };
    assert_eq!(m.ceiling_for(&other), 0);
}

#[test]
fn finds_an_import_and_reports_a_one_based_line() {
    let hits = refs("mod a;\nuse crate::presentation::Foo;\n");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line, 2);
}

#[test]
fn a_fully_qualified_call_is_an_import_too() {
    // The bug this gate had. `use crate::presentation` matched; the same reference written
    // inline did not, so a real upward call passed every push and the gate reported zero.
    let hits = refs("fn f() {\n    let h = crate::presentation::widget::handle();\n}\n");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line, 2);
}

#[test]
fn a_commented_import_is_not_a_violation() {
    let text = "// use crate::presentation::Foo;\n/* use crate::presentation */\n";
    assert!(refs(text).is_empty());
}

#[test]
fn a_multi_line_block_comment_is_blank_all_the_way_through() {
    // The old check tested only whether a LINE started with a comment marker, so an
    // interior line of a block comment read as code.
    let text = "/*\n use crate::presentation::Foo;\n*/\nfn f() {}\n";
    assert!(refs(text).is_empty());
}

#[test]
fn a_nested_block_comment_does_not_end_early() {
    // Rust block comments nest, so the first `*/` closes the inner one only.
    let text = "/* outer /* inner */ crate::presentation::f(); */\nfn f() {}\n";
    assert!(refs(text).is_empty());
}

#[test]
fn a_doc_link_is_a_reference_not_an_import() {
    // Rejecting these would push authors to write worse docs, and they are the entire
    // observed false-positive set in the repo this gate guards.
    let text = "/// See [`crate::presentation::widget`] for the shape.\nfn f() {}\n";
    assert!(refs(text).is_empty());
}

#[test]
fn a_url_in_a_string_does_not_open_a_comment() {
    // `//` inside a string must not blank the rest of the line — that would hide a real
    // violation sitting after it, which is the silent-miss failure this gate exists to avoid.
    let text = "fn f() {\n    let u = \"https://example.test\"; crate::presentation::g();\n}\n";
    assert_eq!(refs(text).len(), 1);
}

#[test]
fn a_lifetime_is_not_a_char_literal() {
    // `'a` must not be read as opening a literal; if it were, everything to the next quote
    // would desynchronise and a following violation could be swallowed.
    let text = "struct W<'a> {\n    r: &'a str,\n}\nfn f() { crate::presentation::g(); }\n";
    let hits = refs(text);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line, 4);
}

#[test]
fn a_braced_use_group_names_the_first_segment_of_every_element() {
    // The nested form is the trap: elements are `presentation::a` and `infrastructure::b`,
    // so comparing whole elements to the layer name finds nothing.
    let flat = refs("use crate::{presentation, infrastructure};\n");
    assert_eq!(flat.len(), 1);
    let nested = refs("use crate::{presentation::a, infrastructure::b};\n");
    assert_eq!(nested.len(), 1);
}

#[test]
fn a_layer_name_that_is_only_a_prefix_is_not_a_hit() {
    // `presentational` starts with `presentation`; a substring test would fire on it.
    assert!(refs("use crate::presentational::Foo;\n").is_empty());
}

#[test]
fn a_reference_is_found_whatever_punctuation_follows_it() {
    for text in [
        "fn f() -> crate::presentation::T { todo!() }\n",
        "fn f(x: Vec<crate::presentation>) {}\n",
        "fn f() { crate::presentation::m!(); }\n",
        "fn f() { g(crate::presentation::V); }\n",
    ] {
        assert_eq!(refs(text).len(), 1, "{text}");
    }
}

#[test]
fn everything_from_the_first_cfg_test_down_is_test_code() {
    let text = "use crate::x;\n#[cfg(test)]\nmod t {\n use crate::presentation::Foo;\n}\n";
    assert!(refs(text).is_empty());
}

#[test]
fn code_above_a_cfg_test_block_is_still_read() {
    assert_eq!(
        refs("use crate::presentation::Foo;\n#[cfg(test)]\nmod t {}\n").len(),
        1
    );
}

#[test]
fn test_files_are_recognised_by_every_convention_the_rules_allow() {
    assert!(is_test_file("src/a/tests.rs"));
    assert!(is_test_file("src/a_tests.rs"));
    assert!(is_test_file("tests/integration.rs"));
    assert!(!is_test_file("src/attestations.rs"));
}
