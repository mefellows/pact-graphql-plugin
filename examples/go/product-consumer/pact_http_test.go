package productconsumer

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/pact-foundation/pact-go/v2/consumer"
	"github.com/pact-foundation/pact-go/v2/matchers"
	"github.com/pact-foundation/pact-graphql-plugin/go/pactgraphql"
)

const productQuery = `
  query GetProduct($id: ID!) {
    product(id: $id) {
      id
      name
      status
    }
  }
`

func TestGraphQLHTTPPactWritesPactFile(t *testing.T) {
	t.Setenv("PACT_GRAPHQL_PLUGIN_VERSION", "0.1.0")

	schema, err := os.ReadFile("schema.graphql")
	if err != nil {
		t.Fatal(err)
	}

	pactDir := "pacts"
	pactFile := filepath.Join(pactDir, "product-consumer-product-provider.json")
	_ = os.Remove(pactFile)

	pact, err := consumer.NewV4Pact(consumer.MockHTTPProviderConfig{
		Consumer: "product-consumer",
		Provider: "product-provider",
		PactDir:  pactDir,
	})
	if err != nil {
		t.Fatal(err)
	}

	variables := map[string]any{"id": "10"}
	interaction, err := pactgraphql.HTTPInteraction(pact, "a GraphQL product request", pactgraphql.GraphQLInteractionConfig{
		Schema:        string(schema),
		Query:         productQuery,
		OperationName: "GetProduct",
		Variables:     variables,
	})
	if err != nil {
		t.Fatal(err)
	}

	err = interaction.
		WillRespondWith(200, func(builder *consumer.V4InteractionWithPluginResponseBuilder) {
			builder.Header("content-type", matchers.String("application/json"))
			builder.JSONBody(map[string]any{
				"data": map[string]any{
					"product": map[string]any{
						"id":     "10",
						"name":   "product name",
						"status": "ACTIVE",
					},
				},
			})
		}).
		ExecuteTest(t, func(config consumer.MockServerConfig) error {
			body, err := pactgraphql.GraphQLRequestBody(productQuery, variables, "GetProduct")
			if err != nil {
				return err
			}

			url := fmt.Sprintf("http://%s:%d/graphql", config.Host, config.Port)
			req, err := http.NewRequest(http.MethodPost, url, bytes.NewReader(body))
			if err != nil {
				return err
			}
			req.Header.Set("content-type", "application/graphql")

			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			_, _ = io.Copy(io.Discard, resp.Body)
			if resp.StatusCode != http.StatusOK {
				return fmt.Errorf("unexpected status %d", resp.StatusCode)
			}
			return nil
		})
	if err != nil {
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

	assertJSONContainsValue(t, pactJSON, "query_document", normalizeQuery(productQuery))
	variablesJSON, err := json.Marshal(variables)
	if err != nil {
		t.Fatal(err)
	}
	assertJSONContainsValue(t, pactJSON, "variables_json", string(variablesJSON))
	assertJSONContainsSchema(t, pactJSON, strings.TrimSpace(string(schema)))
}

func normalizeQuery(query string) string {
	normalized := strings.ReplaceAll(query, "\r\n", "\n")
	normalized = strings.ReplaceAll(normalized, "\r", "\n")
	lines := strings.Split(normalized, "\n")
	minIndent := -1

	for _, line := range lines {
		if strings.TrimSpace(line) == "" {
			continue
		}
		indent := 0
		for _, char := range line {
			if char == ' ' || char == '\t' {
				indent++
				continue
			}
			break
		}
		if minIndent == -1 || indent < minIndent {
			minIndent = indent
		}
	}

	if minIndent < 0 {
		minIndent = 0
	}

	dedented := make([]string, 0, len(lines))
	for _, line := range lines {
		if strings.TrimSpace(line) == "" {
			dedented = append(dedented, "")
			continue
		}
		if len(line) < minIndent {
			dedented = append(dedented, "")
			continue
		}
		dedented = append(dedented, line[minIndent:])
	}

	return strings.TrimSpace(strings.Join(dedented, "\n"))
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

func assertJSONContainsSchema(t *testing.T, payload any, schema string) {
	t.Helper()

	if hasJSONValue(payload, "schema_sdl", schema) {
		return
	}

	if hasBase64Schema(payload, []string{"schema_inline_base64", "base64_sdl"}, schema) {
		return
	}

	t.Fatalf("expected schema to be present")
}

func hasJSONValue(payload any, key string, expected any) bool {
	for _, match := range findJSONValues(payload, key) {
		if reflect.DeepEqual(match, expected) {
			return true
		}
	}
	return false
}

func hasBase64Schema(payload any, keys []string, expected string) bool {
	for _, key := range keys {
		for _, match := range findJSONValues(payload, key) {
			encoded, ok := match.(string)
			if !ok {
				continue
			}
			decoded, err := base64.StdEncoding.DecodeString(encoded)
			if err != nil {
				continue
			}
			if strings.TrimSpace(string(decoded)) == expected {
				return true
			}
		}
	}
	return false
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
