use pact_graphql_plugin::encoder::{GraphqlRequest, RequestEncoder};

#[test]
fn encodes_json_payload() {
    let req = GraphqlRequest::json("query { ping }", None::<String>, None::<String>);
    let encoded = RequestEncoder::encode(&req).unwrap();

    assert_eq!(
        encoded.body.as_deref(),
        Some(r#"{"query":"query { ping }"}"#)
    );
    assert!(encoded.query_string.is_none());
}

#[test]
fn encodes_query_string_payload() {
    let req = GraphqlRequest::query_string("query { pong }", None::<String>, None::<String>);
    let encoded = RequestEncoder::encode(&req).unwrap();

    assert!(encoded.body.is_none());
    assert_eq!(
        encoded.query_string.as_deref(),
        Some("query=query+%7B+pong+%7D")
    );
}
