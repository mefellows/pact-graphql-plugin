package productconsumer

import (
	"testing"

	"github.com/pact-foundation/pact-graphql-plugin/go/pactgraphql"
)

func TestGraphQLHelperWiresModule(t *testing.T) {
	_, err := pactgraphql.GraphQLRequestBody("query { product(id: \"1\") { id } }", nil, "")
	if err != nil {
		t.Fatalf("expected graphql helper to build request body: %v", err)
	}
}
