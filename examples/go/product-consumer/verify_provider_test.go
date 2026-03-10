//go:build provider
// +build provider

package productconsumer

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/pact-foundation/pact-go/v2/message"
	"github.com/pact-foundation/pact-go/v2/models"
	"github.com/pact-foundation/pact-go/v2/provider"
)

func TestBuildInventoryChangedEvent(t *testing.T) {
	event, err := buildInventoryChangedEvent("var-1")
	if err != nil {
		t.Fatal(err)
	}
	if event.Subscription != "InventoryChanged" {
		t.Fatalf("unexpected subscription: %s", event.Subscription)
	}
}

func TestVerifyGraphQLProvider(t *testing.T) {
	server, err := startProviderServer()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = server.close(context.Background())
	})

	verifier := provider.NewVerifier()

	httpPact := filepath.Join("pacts", "product-consumer-product-provider.json")
	messagePact := filepath.Join("pacts", "messages", "product-consumer-product-provider.json")

	if err := verifier.VerifyProvider(t, provider.VerifyRequest{
		ProviderBaseURL: server.url,
		Provider:        "product-provider",
		PactFiles:       []string{httpPact},
	}); err != nil {
		t.Fatal(err)
	}

	messageHandlers := message.Handlers{
		"an inventory change message": func([]models.ProviderState) (message.Body, message.Metadata, error) {
			event, err := buildInventoryChangedEvent("var-1")
			if err != nil {
				return nil, nil, err
			}
			return event, message.Metadata{"contentType": "application/json"}, nil
		},
	}

	if err := verifier.VerifyProvider(t, provider.VerifyRequest{
		Provider:        "product-provider",
		PactFiles:       []string{messagePact},
		MessageHandlers: messageHandlers,
	}); err != nil {
		t.Fatal(err)
	}
}
