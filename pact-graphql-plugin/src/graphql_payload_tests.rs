use super::*;

#[test]
fn canonicalize_query_falls_back_to_dedented_text_on_syntax_error() {
    let input = "query Q { unclosed";
    let result = canonicalize_query(input, Some("Q")).unwrap();
    assert_eq!(result, dedent_and_trim(input));
}
