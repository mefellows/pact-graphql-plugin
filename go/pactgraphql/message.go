package pactgraphql

import (
	"encoding/json"
	"errors"
	"os"

	"github.com/pact-foundation/pact-graphql-plugin/go/pactgraphql/pact"
)

type GraphQLMessageConfig struct {
	Schema        string
	Subscription  string
	OperationName string
	Variables     any
	Data          any
}

func MessageInteraction(p *pact.MessagePact, name string, cfg GraphQLMessageConfig) (*pact.MessageInteraction, error) {
	if cfg.Data == nil {
		return nil, errors.New("GraphQL message data is required")
	}

	config, err := buildGraphQLConfiguration(GraphQLInteractionConfig{
		Schema:        cfg.Schema,
		Query:         cfg.Subscription,
		OperationName: cfg.OperationName,
		Variables:     cfg.Variables,
	})
	if err != nil {
		return nil, err
	}

	contents, err := json.Marshal(config)
	if err != nil {
		return nil, err
	}

	variables, err := buildEnvelopeVariables(cfg.Variables)
	if err != nil {
		return nil, err
	}

	envelope := map[string]any{
		"subscription": cfg.OperationName,
		"data":         cfg.Data,
	}
	if variables != nil {
		envelope["variables"] = variables
	}

	version := os.Getenv("PACT_GRAPHQL_PLUGIN_VERSION")
	if version == "" {
		version = "0.0.0"
	}

	interaction := p.AddAsynchronousMessage()
	unconfigured := interaction.ExpectsToReceive(name)
	pluginInteraction := unconfigured.UsingPlugin(pact.MessagePluginConfig{Plugin: "graphql", Version: version})
	pluginContents := pluginInteraction.WithContents(string(contents), "application/graphql")
	unconfigured.WithJSONContent(envelope)

	return pluginContents, nil
}
