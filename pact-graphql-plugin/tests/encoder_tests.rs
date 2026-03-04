use pact_graphql_plugin::encoder::{GraphqlRequest, RequestEncoder};

#[test]
fn encodes_json_payload() {
    let req = GraphqlRequest::json("query { ping }", None, None);
    let encoded = RequestEncoder::encode(&req).unwrap();

    assert_eq!(
        encoded.body.as_deref(),
        Some(r#"{"query":"query { ping }"}"#)
    );
    assert!(encoded.query_string.is_none());
}

#[test]
fn encodes_json_payload_preserving_field_order() {
    let variables = serde_json::json!({ "id": 10 }).to_string();
    let req = GraphqlRequest::json(
        "query { ping }",
        Some("PingQuery".to_string()),
        Some(variables),
    );
    let encoded = RequestEncoder::encode(&req).unwrap();

    assert_eq!(
        encoded.body.as_deref(),
        Some(r#"{"query":"query { ping }","operationName":"PingQuery","variables":{"id":10}}"#,)
    );
    assert!(encoded.query_string.is_none());
}

#[test]
fn encodes_query_string_payload() {
    let req = GraphqlRequest::query_string("query { pong }", None, None);
    let encoded = RequestEncoder::encode(&req).unwrap();

    assert!(encoded.body.is_none());
    assert_eq!(
        encoded.query_string.as_deref(),
        Some("query=query+%7B+pong+%7D")
    );
}

#[test]
fn encodes_query_string_with_operation_and_variables() {
    let pretty_variables = serde_json::to_string_pretty(&serde_json::json!({ "id": 10 })).unwrap();
    let req = GraphqlRequest::query_string(
        "query { ping }",
        Some("PingQuery".to_string()),
        Some(pretty_variables),
    );
    let encoded = RequestEncoder::encode(&req).unwrap();

    assert!(encoded.body.is_none());
    assert_eq!(
        encoded.query_string.as_deref(),
        Some("query=query+%7B+ping+%7D&operationName=PingQuery&variables=%7B%22id%22%3A10%7D")
    );
}

#[test]
fn query_string_variables_invalid_json_errors() {
    let req = GraphqlRequest::query_string("query { ping }", None, Some("{".to_string()));
    let err = RequestEncoder::encode(&req).unwrap_err();

    assert!(err
        .to_string()
        .contains("variables_json must be valid JSON"));
}
