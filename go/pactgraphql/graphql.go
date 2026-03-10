package pactgraphql

import (
	"encoding/json"
	"errors"
	"strings"
)

type GraphQLTransport string

const (
	GraphQLTransportJSONBody    GraphQLTransport = "json_body"
	GraphQLTransportQueryString GraphQLTransport = "query_string"
)

type GraphQLRequestOptions struct {
	Query         string
	Variables     any
	OperationName string
}

type GraphQLInteractionConfig struct {
	Schema        string
	Query         string
	OperationName string
	Variables     any
	Transport     GraphQLTransport
}

type graphQLRequest struct {
	Query         string `json:"query"`
	Variables     any    `json:"variables,omitempty"`
	OperationName string `json:"operationName,omitempty"`
}

type graphQLPluginConfiguration struct {
	QueryDocument string `json:"query_document"`
	OperationName string `json:"operation_name,omitempty"`
	VariablesJSON string `json:"variables_json,omitempty"`
	Transport     string `json:"transport"`
	SchemaSDL     string `json:"schema_sdl,omitempty"`
}

func GraphQLRequestBody(query string, variables any, operationName string) ([]byte, error) {
	payload, err := buildGraphQLRequestBody(GraphQLRequestOptions{
		Query:         query,
		Variables:     variables,
		OperationName: operationName,
	})
	if err != nil {
		return nil, err
	}
	return json.Marshal(payload)
}

func buildGraphQLRequestBody(options GraphQLRequestOptions) (graphQLRequest, error) {
	if strings.TrimSpace(options.Query) == "" {
		return graphQLRequest{}, errors.New("GraphQL query is required")
	}

	variables, err := buildEnvelopeVariables(options.Variables)
	if err != nil {
		return graphQLRequest{}, err
	}

	return graphQLRequest{
		Query:         normalizeQuery(options.Query),
		Variables:     variables,
		OperationName: options.OperationName,
	}, nil
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

func serializeVariables(variables any) (string, error) {
	if variables == nil {
		return "", nil
	}

	if raw, ok := variables.(string); ok {
		var parsed any
		if err := json.Unmarshal([]byte(raw), &parsed); err != nil {
			return "", errors.New("variables string must contain valid JSON")
		}
		return raw, nil
	}

	encoded, err := json.Marshal(variables)
	if err != nil {
		return "", errors.New("variables value must be JSON serialisable")
	}

	return string(encoded), nil
}

func buildEnvelopeVariables(variables any) (any, error) {
	if variables == nil {
		return nil, nil
	}

	if raw, ok := variables.(string); ok {
		var parsed any
		if err := json.Unmarshal([]byte(raw), &parsed); err != nil {
			return nil, errors.New("variables string must contain valid JSON")
		}
		return parsed, nil
	}

	return variables, nil
}

func resolveTransport(transport GraphQLTransport) GraphQLTransport {
	if transport == "" {
		return GraphQLTransportJSONBody
	}
	return transport
}

func buildGraphQLConfiguration(config GraphQLInteractionConfig) (graphQLPluginConfiguration, error) {
	if strings.TrimSpace(config.Query) == "" {
		return graphQLPluginConfiguration{}, errors.New("GraphQL query is required")
	}

	variablesJSON, err := serializeVariables(config.Variables)
	if err != nil {
		return graphQLPluginConfiguration{}, err
	}

	return graphQLPluginConfiguration{
		QueryDocument: normalizeQuery(config.Query),
		OperationName: config.OperationName,
		VariablesJSON: variablesJSON,
		Transport:     string(resolveTransport(config.Transport)),
		SchemaSDL:     config.Schema,
	}, nil
}
