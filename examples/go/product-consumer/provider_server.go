package productconsumer

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
)

type graphqlRequest struct {
	Query         string         `json:"query"`
	Variables     map[string]any `json:"variables"`
	OperationName string         `json:"operationName"`
}

type providerServer struct {
	url    string
	server *http.Server
}

type inventoryEvent struct {
	Subscription string         `json:"subscription"`
	Variables    map[string]any `json:"variables,omitempty"`
	Data         map[string]any `json:"data"`
}

func buildInventoryChangedEvent(variantID string) (inventoryEvent, error) {
	variant, ok := providerData.variants[variantID]
	if !ok {
		return inventoryEvent{}, errors.New("variant not found")
	}

	return inventoryEvent{
		Subscription: "InventoryChanged",
		Variables:    map[string]any{"variantId": variantID},
		Data: map[string]any{
			"inventoryChanged": map[string]any{
				"quantity":  variant.Inventory.Quantity,
				"updatedAt": variant.Inventory.UpdatedAt,
			},
		},
	}, nil
}

func startProviderServer() (*providerServer, error) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, err
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/graphql", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			w.WriteHeader(http.StatusMethodNotAllowed)
			return
		}

		var payload graphqlRequest
		if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
			w.WriteHeader(http.StatusBadRequest)
			_, _ = w.Write([]byte(`{"error":"invalid JSON"}`))
			return
		}

		response, err := buildResponse(payload)
		if err != nil {
			w.WriteHeader(http.StatusBadRequest)
			_, _ = w.Write([]byte(fmt.Sprintf(`{"error":%q}`, err.Error())))
			return
		}

		w.Header().Set("content-type", "application/json")
		_ = json.NewEncoder(w).Encode(response)
	})

	server := &http.Server{Handler: mux}
	go func() {
		_ = server.Serve(listener)
	}()

	return &providerServer{
		url:    fmt.Sprintf("http://%s", listener.Addr().String()),
		server: server,
	}, nil
}

func (s *providerServer) close(ctx context.Context) error {
	return s.server.Shutdown(ctx)
}

type providerInventory struct {
	Quantity  int    `json:"quantity"`
	UpdatedAt string `json:"updatedAt"`
}

type providerVariant struct {
	ID        string            `json:"id"`
	SKU       string            `json:"sku"`
	Price     map[string]any    `json:"price"`
	Inventory providerInventory `json:"inventory"`
}

type providerProduct struct {
	ID       string
	SKU      string
	Name     string
	Status   string
	Category map[string]any
	Variants []providerVariant
	Reviews  []map[string]any
}

type providerState struct {
	products map[string]providerProduct
	variants map[string]providerVariant
}

var providerData = providerState{
	products: map[string]providerProduct{
		"prod-1": {
			ID:     "prod-1",
			SKU:    "SKU-TRAIL-001",
			Name:   "Trail Backpack",
			Status: "ACTIVE",
			Category: map[string]any{
				"id":   "cat-1",
				"name": "Bags",
			},
			Variants: []providerVariant{
				{
					ID:  "var-1",
					SKU: "SKU-TRAIL-001",
					Price: map[string]any{
						"list": map[string]any{
							"amount":   129.99,
							"currency": "USD",
						},
					},
					Inventory: providerInventory{
						Quantity:  42,
						UpdatedAt: "2026-03-08T12:00:00Z",
					},
				},
			},
			Reviews: []map[string]any{
				{
					"id":     "rev-1",
					"rating": "FIVE",
					"author": map[string]any{
						"id":   "cust-1",
						"name": "Alex",
					},
				},
			},
		},
		"prod-2": {
			ID:     "prod-2",
			SKU:    "SKU-WEEK-004",
			Name:   "Weekender Tote",
			Status: "DRAFT",
		},
	},
	variants: map[string]providerVariant{
		"var-1": {
			ID:  "var-1",
			SKU: "SKU-TRAIL-001",
			Price: map[string]any{
				"list": map[string]any{
					"amount":   129.99,
					"currency": "USD",
				},
			},
			Inventory: providerInventory{
				Quantity:  42,
				UpdatedAt: "2026-03-08T12:00:00Z",
			},
		},
		"var-2": {
			ID:  "var-2",
			SKU: "SKU-WEEK-004",
			Price: map[string]any{
				"list": map[string]any{
					"amount":   159.99,
					"currency": "USD",
				},
			},
			Inventory: providerInventory{
				Quantity:  18,
				UpdatedAt: "2026-03-08T12:00:00Z",
			},
		},
	},
}

func buildResponse(request graphqlRequest) (map[string]any, error) {
	switch request.OperationName {
	case "GetProduct":
		return map[string]any{
			"data": map[string]any{
				"product": map[string]any{
					"id":     "10",
					"name":   "product name",
					"status": "ACTIVE",
				},
			},
		}, nil
	case "ProductsByStatus":
		product := providerData.products["prod-1"]
		return map[string]any{
			"data": map[string]any{
				"products": []map[string]any{
					{
						"id":       product.ID,
						"name":     product.Name,
						"status":   product.Status,
						"category": product.Category,
						"variants": []map[string]any{
							{
								"id":    product.Variants[0].ID,
								"sku":   product.Variants[0].SKU,
								"price": product.Variants[0].Price,
								"inventory": map[string]any{
									"quantity":  product.Variants[0].Inventory.Quantity,
									"updatedAt": product.Variants[0].Inventory.UpdatedAt,
								},
							},
						},
						"reviews": product.Reviews,
					},
				},
			},
		}, nil
	case "PlaceOrder":
		return map[string]any{
			"data": map[string]any{
				"placeOrder": map[string]any{
					"id":     "order-1",
					"status": "PLACED",
					"total": map[string]any{
						"amount":   289.97,
						"currency": "USD",
					},
					"lines": []map[string]any{
						{
							"quantity": 2,
							"product": map[string]any{
								"id":   "prod-1",
								"name": "Trail Backpack",
							},
							"variant": map[string]any{
								"id":  "var-1",
								"sku": "SKU-TRAIL-001",
							},
						},
						{
							"quantity": 1,
							"product": map[string]any{
								"id":   "prod-2",
								"name": "Weekender Tote",
							},
							"variant": map[string]any{
								"id":  "var-2",
								"sku": "SKU-WEEK-004",
							},
						},
					},
					"shipments": []map[string]any{
						{
							"id":     "ship-1",
							"status": "PENDING",
							"address": map[string]any{
								"city":    "San Francisco",
								"country": "US",
							},
						},
					},
				},
			},
		}, nil
	default:
		return nil, fmt.Errorf("unsupported operation %q", request.OperationName)
	}
}
