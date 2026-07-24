use super::*;

fn diff_queries(
    expected: &str,
    actual: &str,
    operation_name: Option<&str>,
) -> anyhow::Result<Vec<QueryDiff>> {
    let expected = parse_and_inline(expected, operation_name)?;
    let actual = parse_and_inline(actual, operation_name)?;
    Ok(diff_operations(&expected, &actual))
}

fn diff_queries_with(
    expected: &str,
    actual: &str,
    operation_name: Option<&str>,
    mode: QueryMatching,
) -> anyhow::Result<Vec<QueryDiff>> {
    if mode == QueryMatching::Exact {
        return Ok(diff_exact(expected, actual));
    }
    let expected_op = parse_and_inline(expected, operation_name)?;
    let actual_op = parse_and_inline(actual, operation_name)?;
    Ok(diff_operations_with(&expected_op, &actual_op, mode))
}

#[test]
fn identical_queries_produce_no_diff() {
    let diffs = diff_queries(
        "query Q { product { id name } }",
        "query Q {\n  product {\n    id\n    name\n  }\n}",
        Some("Q"),
    )
    .unwrap();
    assert!(diffs.is_empty(), "expected no diffs, got {diffs:#?}");
}

#[test]
fn reports_the_path_of_a_missing_field() {
    let diffs = diff_queries(
        "query Q { product { id name status } }",
        "query Q { product { id name } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.status");
    assert!(diffs[0].description.contains("not selected"), "{}", diffs[0].description);
}

#[test]
fn reports_the_path_of_an_unexpected_field() {
    let diffs = diff_queries(
        "query Q { product { id } }",
        "query Q { product { id name } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.name");
    assert!(diffs[0].description.contains("not expected"), "{}", diffs[0].description);
}

#[test]
fn reports_nested_paths() {
    let diffs = diff_queries(
        "query Q { product { category { id name } } }",
        "query Q { product { category { id } } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.category.name");
}

#[test]
fn reports_argument_differences() {
    let diffs = diff_queries(
        "query Q { product(id: \"10\") { id } }",
        "query Q { product(id: \"11\") { id } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product");
    assert!(diffs[0].description.contains("argument"), "{}", diffs[0].description);
    assert!(diffs[0].expected.contains("10"));
    assert!(diffs[0].actual.contains("11"));
}

#[test]
fn reports_operation_type_differences() {
    let diffs = diff_queries(
        "query Q { thing { id } }",
        "mutation Q { thing { id } }",
        Some("Q"),
    )
    .unwrap();
    assert!(
        diffs.iter().any(|d| d.description.contains("operation type")),
        "got {diffs:#?}"
    );
}

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

#[test]
fn rejects_a_self_referencing_fragment() {
    let err = canonical_document(
        "query Q { product { ...Loop } }\nfragment Loop on Product { id ...Loop }",
        Some("Q"),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("fragment `Loop` forms a cycle"),
        "expected a cycle diagnostic, got: {err}"
    );
}

#[test]
fn rejects_a_mutually_recursive_fragment_pair() {
    let err = canonical_document(
        "query Q { product { ...A } }\n\
         fragment A on Product { id ...B }\n\
         fragment B on Product { name ...A }",
        Some("Q"),
    )
    .unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("forms a cycle") && message.contains("A -> B -> A"),
        "expected the cycle path in the message, got: {message}"
    );
}

#[test]
fn allows_the_same_fragment_spread_twice_in_sequence() {
    // Two sibling spreads of one fragment are not a cycle — the stack must pop
    // between them, or this regresses to a false positive.
    let out = canonical_document(
        "query Q { a { ...F } b { ...F } }\nfragment F on Thing { id }",
        Some("Q"),
    )
    .expect("sibling spreads of the same fragment are legal");
    assert_eq!(out.matches("id").count(), 2, "both spreads expanded: {out}");
}

const EXPECTED: &str = "query Q { product { id name status } }";
const FEWER: &str = "query Q { product { id name } }";
const MORE: &str = "query Q { product { id name status category { id } } }";

#[test]
fn subset_allows_the_actual_query_to_request_fewer_fields() {
    let diffs = diff_queries_with(EXPECTED, FEWER, Some("Q"), QueryMatching::Subset).unwrap();
    assert!(diffs.is_empty(), "got {diffs:#?}");
}

#[test]
fn subset_still_rejects_unexpected_fields() {
    let diffs = diff_queries_with(EXPECTED, MORE, Some("Q"), QueryMatching::Subset).unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.category");
}

#[test]
fn semantic_rejects_fewer_fields() {
    let diffs = diff_queries_with(EXPECTED, FEWER, Some("Q"), QueryMatching::Semantic).unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.status");
}

#[test]
fn exact_rejects_a_reformatted_but_equivalent_query() {
    let reformatted = "query Q {\n  product {\n    id\n    name\n    status\n  }\n}";
    let semantic =
        diff_queries_with(EXPECTED, reformatted, Some("Q"), QueryMatching::Semantic).unwrap();
    assert!(semantic.is_empty(), "semantic tolerates formatting: {semantic:#?}");

    let exact = diff_queries_with(EXPECTED, reformatted, Some("Q"), QueryMatching::Exact).unwrap();
    assert_eq!(exact.len(), 1, "exact does not: {exact:#?}");
    assert!(exact[0].description.contains("byte-for-byte"), "{}", exact[0].description);
}
