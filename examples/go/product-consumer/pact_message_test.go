package productconsumer

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	message "github.com/pact-foundation/pact-go/v2/message/v4"
	"github.com/pact-foundation/pact-graphql-plugin/go/pactgraphql"
)

const inventorySubscription = `
  subscription InventoryChanged($variantId: ID!) {
    inventoryChanged(variantId: $variantId) {
      quantity
      updatedAt
    }
  }
`

func TestGraphQLMessagePactWritesPactFile(t *testing.T) {
	t.Setenv("PACT_GRAPHQL_PLUGIN_VERSION", "0.1.0")

	schema, err := os.ReadFile("schema.graphql")
	if err != nil {
		t.Fatal(err)
	}

	pactDir := t.TempDir()
	pactFile := filepath.Join(pactDir, "product-consumer-product-provider.json")

	pact, err := message.NewAsynchronousPact(message.Config{
		Consumer: "product-consumer",
		Provider: "product-provider",
		PactDir:  pactDir,
	})
	if err != nil {
		t.Fatal(err)
	}

	variables := map[string]any{"variantId": "var-1"}
	data := map[string]any{
		"inventoryChanged": map[string]any{
			"quantity":  42,
			"updatedAt": "2026-03-08T12:00:00Z",
		},
	}

	interaction, err := pactgraphql.MessageInteraction(pact, "an inventory change message", pactgraphql.GraphQLMessageConfig{
		Schema:        string(schema),
		Subscription:  inventorySubscription,
		OperationName: "InventoryChanged",
		Variables:     variables,
		Data:          data,
	})
	if err != nil {
		t.Fatal(err)
	}

	if err := interaction.ExecuteTest(t, func(_ message.AsynchronousMessage) error { return nil }); err != nil {
		t.Fatal(err)
	}
	payload, err := os.ReadFile(pactFile)
	if err != nil {
		t.Fatal(err)
	}

	var pactJSON any
	if err := json.Unmarshal(payload, &pactJSON); err != nil {
		t.Fatal(err)
	}

	assertJSONContainsSchema(t, pactJSON, strings.TrimSpace(string(schema)))
	assertJSONContainsValue(t, pactJSON, "query_document", normalizeQuery(inventorySubscription))
	variablesJSON, err := json.Marshal(variables)
	if err != nil {
		t.Fatal(err)
	}
	assertJSONContainsValue(t, pactJSON, "variables_json", string(variablesJSON))
	assertJSONContainsValue(t, pactJSON, "subscription", "InventoryChanged")
	assertJSONContainsValue(t, pactJSON, "variables", variables)
	assertJSONContainsValue(t, pactJSON, "data", map[string]any{
		"inventoryChanged": map[string]any{
			"quantity":  float64(42),
			"updatedAt": "2026-03-08T12:00:00Z",
		},
	})
}
