use super::*;

#[test]
fn ignores_insignificant_whitespace() {
    let a = canonical_document("query Q { product(id: $id) { id name } }", Some("Q")).unwrap();
    let b = canonical_document(
        "query Q {\n  product( id : $id ) {\n    id\n    name\n  }\n}",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn ignores_comments() {
    let a = canonical_document("query Q { product { id } }", Some("Q")).unwrap();
    let b = canonical_document(
        "# fetch the product\nquery Q {\n  product { id } # just the id\n}",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn inlines_fragment_spreads() {
    let with_fragment = canonical_document(
        "query Q { product { ...Fields } }\nfragment Fields on Product { id name }",
        Some("Q"),
    )
    .unwrap();
    let inlined = canonical_document("query Q { product { id name } }", Some("Q")).unwrap();
    assert_eq!(with_fragment, inlined);
}

#[test]
fn preserves_field_order() {
    let a = canonical_document("query Q { product { id name } }", Some("Q")).unwrap();
    let b = canonical_document("query Q { product { name id } }", Some("Q")).unwrap();
    assert_ne!(a, b, "field order is significant in GraphQL responses");
}

#[test]
fn preserves_aliases_and_arguments() {
    let out = canonical_document(
        "query Q { mine: product(id: \"10\", first: 5) { id } }",
        Some("Q"),
    )
    .unwrap();
    assert!(out.contains("mine: product"), "alias survives: {out}");
    assert!(out.contains("id: \"10\""), "arguments survive: {out}");
}

#[test]
fn rejects_an_unknown_operation_name() {
    let err = canonical_document("query Q { product { id } }", Some("Other")).unwrap_err();
    assert!(
        err.to_string().contains("Other"),
        "error names the missing operation: {err}"
    );
}

#[test]
fn rejects_an_unresolvable_fragment() {
    let err = canonical_document("query Q { product { ...Missing } }", Some("Q")).unwrap_err();
    assert!(
        err.to_string().contains("Missing"),
        "error names the missing fragment: {err}"
    );
}
