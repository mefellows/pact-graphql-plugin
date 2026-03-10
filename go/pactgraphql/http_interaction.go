package pactgraphql

import (
	"encoding/json"
	"os"

	"github.com/pact-foundation/pact-go/v2/matchers"

	"github.com/pact-foundation/pact-graphql-plugin/go/pactgraphql/pact"
)

func HTTPInteraction(p *pact.V4, name string, cfg GraphQLInteractionConfig) (*pact.Interaction, error) {
	config, err := buildGraphQLConfiguration(cfg)
	if err != nil {
		return nil, err
	}

	contents, err := json.Marshal(config)
	if err != nil {
		return nil, err
	}

	version := os.Getenv("PACT_GRAPHQL_PLUGIN_VERSION")
	if version == "" {
		version = "0.0.0"
	}

	interaction := p.AddInteraction().
		UponReceiving(name).
		UsingPlugin(pact.PluginConfig{Plugin: "graphql", Version: version}).
		WithRequest("POST", "/graphql", func(builder *pact.V4InteractionWithPluginRequestBuilder) {
			builder.Header("content-type", matchers.String("application/graphql"))
			builder.PluginContents("application/graphql", string(contents))
		})

	return interaction, nil
}
