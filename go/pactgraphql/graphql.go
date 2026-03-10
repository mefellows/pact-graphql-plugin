package pactgraphql

import "encoding/json"

type graphQLRequest struct {
	Query         string         `json:"query"`
	Variables     map[string]any `json:"variables,omitempty"`
	OperationName string         `json:"operationName,omitempty"`
}

func GraphQLRequestBody(query string, variables map[string]any, operationName string) ([]byte, error) {
	payload := graphQLRequest{
		Query:         query,
		Variables:     variables,
		OperationName: operationName,
	}
	return json.Marshal(payload)
}
