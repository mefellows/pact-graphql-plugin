package pactgraphql

import "testing"

func TestGraphQLPluginConfiguration(t *testing.T) {
	config, err := buildGraphQLConfiguration(GraphQLInteractionConfig{
		Schema: "type Query { ping(id: ID!): Ping } type Ping { id: ID! }",
		Query: `
      query Ping($id: ID!) {
        ping(id: $id) { id }
      }
    `,
		OperationName: "PingQuery",
		Variables:     map[string]any{"id": "10"},
	})
	if err != nil {
		t.Fatal(err)
	}

	if config.SchemaSDL == "" {
		t.Fatalf("expected schema_sdl to be set")
	}
	if config.QueryDocument == "" {
		t.Fatalf("expected query_document to be set")
	}
	if config.VariablesJSON == "" {
		t.Fatalf("expected variables_json to be set")
	}
}

func TestGraphQLPluginConfigurationErrorsOnEmptyQuery(t *testing.T) {
	_, err := buildGraphQLConfiguration(GraphQLInteractionConfig{Query: "   "})
	if err == nil {
		t.Fatalf("expected error for empty query")
	}
}

func TestGraphQLPluginConfigurationErrorsOnInvalidVariables(t *testing.T) {
	_, err := buildGraphQLConfiguration(GraphQLInteractionConfig{
		Query:     "query { ping }",
		Variables: "{invalid}",
	})
	if err == nil {
		t.Fatalf("expected error for invalid variables")
	}
}
