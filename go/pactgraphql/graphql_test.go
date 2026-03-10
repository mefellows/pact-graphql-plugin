package pactgraphql

import (
	"encoding/json"
	"testing"
)

func TestGraphQLRequestBody(t *testing.T) {
	body, err := GraphQLRequestBody(`
    query Ping($id: ID!) {
      ping(id: $id) { id }
    }
  `, `{"id":"10"}`, "PingQuery")
	if err != nil {
		t.Fatal(err)
	}

	var payload map[string]any
	if err := json.Unmarshal(body, &payload); err != nil {
		t.Fatal(err)
	}

	if payload["query"] != "query Ping($id: ID!) {\n  ping(id: $id) { id }\n}" {
		t.Fatalf("unexpected query: %v", payload["query"])
	}
	if payload["operationName"] != "PingQuery" {
		t.Fatalf("unexpected operationName: %v", payload["operationName"])
	}

	vars, ok := payload["variables"].(map[string]any)
	if !ok {
		t.Fatalf("expected variables map, got %T", payload["variables"])
	}
	if vars["id"] != "10" {
		t.Fatalf("unexpected variables: %v", vars)
	}
}
