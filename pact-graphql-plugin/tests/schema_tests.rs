use pact_graphql_plugin::SchemaRegistry;

#[test]
fn stores_and_retrieves_schema() {
    let dir = tempfile::tempdir().unwrap();
    let registry = SchemaRegistry::new(dir.path()).unwrap();
    let reference = registry.store("type Query { ping: String }").unwrap();

    assert_eq!(reference.hash.len(), 64);
    assert_eq!(reference.encoding, "utf-8");

    let inline = registry.inline_schema(&reference.hash).unwrap();
    assert!(inline.contains("type Query"));
}
