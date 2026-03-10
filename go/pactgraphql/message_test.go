package pactgraphql

import (
	"encoding/base64"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"unsafe"

	message "github.com/pact-foundation/pact-go/v2/message/v4"
)

func TestMessageInteractionBuildsEnvelopeAndPluginContents(t *testing.T) {
	pactDir := t.TempDir()
	pact, err := message.NewAsynchronousPact(message.Config{
		Consumer: "product-consumer",
		Provider: "product-provider",
		PactDir:  pactDir,
	})
	if err != nil {
		t.Fatal(err)
	}

	subscription := `
    subscription InventoryChanged($variantId: ID!) {
      inventoryChanged(variantId: $variantId) {
        quantity
        updatedAt
      }
    }
  `
	data := map[string]any{
		"inventoryChanged": map[string]any{
			"quantity":  42,
			"updatedAt": "2026-03-08T12:00:00Z",
		},
	}
	expectedData := map[string]any{
		"inventoryChanged": map[string]any{
			"quantity":  float64(42),
			"updatedAt": "2026-03-08T12:00:00Z",
		},
	}

	schema := "schema { query: Query subscription: Subscription } type Query { _empty: String } type Subscription { inventoryChanged(variantId: ID!): Inventory } type Inventory { quantity: Int! updatedAt: String! }"
	interaction, err := MessageInteraction(pact, "inventory changed", GraphQLMessageConfig{
		Schema:        schema,
		Subscription:  subscription,
		OperationName: "InventoryChanged",
		Variables:     map[string]any{"variantId": "var-1"},
		Data:          data,
	})
	if err != nil {
		t.Fatal(err)
	}
	if interaction == nil {
		t.Fatal("expected interaction")
	}

	pactFile := writeMessagePactFile(t, pact, pactDir)
	payload, err := os.ReadFile(pactFile)
	if err != nil {
		t.Fatal(err)
	}

	var pactJSON any
	if err := json.Unmarshal(payload, &pactJSON); err != nil {
		t.Fatal(err)
	}

	assertJSONContainsBase64(t, pactJSON, []string{"schema_inline_base64", "base64_sdl"}, schema)
	assertJSONContainsValue(t, pactJSON, "query_document", normalizeQuery(subscription))

	variablesJSON, err := serializeVariables(map[string]any{"variantId": "var-1"})
	if err != nil {
		t.Fatal(err)
	}
	assertJSONContainsValue(t, pactJSON, "variables_json", variablesJSON)

	assertJSONContainsValue(t, pactJSON, "subscription", "InventoryChanged")
	assertJSONContainsValue(t, pactJSON, "variables", map[string]any{"variantId": "var-1"})
	assertJSONContainsValue(t, pactJSON, "data", expectedData)
}

func writeMessagePactFile(t *testing.T, pact *message.AsynchronousPact, pactDir string) string {
	t.Helper()

	pactValue := reflect.ValueOf(pact).Elem()
	messageServerField := pactValue.FieldByName("messageserver")
	if !messageServerField.IsValid() {
		t.Fatal("expected message server field")
	}

	messageServer := reflect.NewAt(messageServerField.Type(), unsafe.Pointer(messageServerField.UnsafeAddr())).Elem()
	writeMethod := messageServer.MethodByName("WritePactFile")
	if !writeMethod.IsValid() {
		t.Fatal("expected WritePactFile method")
	}

	results := writeMethod.Call([]reflect.Value{reflect.ValueOf(pactDir), reflect.ValueOf(false)})
	if len(results) == 1 && !results[0].IsNil() {
		if err, ok := results[0].Interface().(error); ok {
			t.Fatalf("write pact file: %v", err)
		}
		t.Fatal("write pact file failed")
	}

	entries, err := os.ReadDir(pactDir)
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if strings.HasSuffix(entry.Name(), ".json") {
			return filepath.Join(pactDir, entry.Name())
		}
	}

	t.Fatal("expected pact file")
	return ""
}

func assertJSONContainsValue(t *testing.T, payload any, key string, expected any) {
	t.Helper()

	matches := findJSONValues(payload, key)
	if len(matches) == 0 {
		t.Fatalf("expected key %q to be present", key)
	}

	for _, match := range matches {
		if reflect.DeepEqual(match, expected) {
			return
		}
	}

	t.Fatalf("expected key %q to contain %v, got %v", key, expected, matches)
}

func assertJSONContainsAnyValue(t *testing.T, payload any, keys []string, expected any) {
	t.Helper()

	var matches []any
	for _, key := range keys {
		matches = append(matches, findJSONValues(payload, key)...)
	}
	if len(matches) == 0 {
		t.Fatalf("expected keys %v to be present", keys)
	}

	for _, match := range matches {
		if reflect.DeepEqual(match, expected) {
			return
		}
	}

	t.Fatalf("expected keys %v to contain %v, got %v", keys, expected, matches)
}

func assertJSONContainsBase64(t *testing.T, payload any, keys []string, expected string) {
	t.Helper()

	var matches []any
	for _, key := range keys {
		matches = append(matches, findJSONValues(payload, key)...)
	}
	if len(matches) == 0 {
		t.Fatalf("expected keys %v to be present", keys)
	}

	for _, match := range matches {
		encoded, ok := match.(string)
		if !ok {
			continue
		}
		decoded, err := base64.StdEncoding.DecodeString(encoded)
		if err != nil {
			continue
		}
		if strings.TrimSpace(string(decoded)) == expected {
			return
		}
	}

	t.Fatalf("expected keys %v to contain base64 schema %q", keys, expected)
}

func findJSONValues(payload any, key string) []any {
	var matches []any
	var walk func(any)

	walk = func(value any) {
		switch v := value.(type) {
		case map[string]any:
			for k, child := range v {
				if k == key {
					matches = append(matches, child)
				}
				walk(child)
			}
		case []any:
			for _, child := range v {
				walk(child)
			}
		}
	}

	walk(payload)
	return matches
}
