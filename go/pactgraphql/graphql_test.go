package pactgraphql

import (
	"strings"
	"testing"
)

func TestGraphQLRequestBody(t *testing.T) {
	body, err := GraphQLRequestBody("query { __typename }", nil, "")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(body), "__typename") {
		t.Fatalf("missing query")
	}
}
