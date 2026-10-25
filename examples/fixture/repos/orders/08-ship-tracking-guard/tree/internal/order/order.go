// Package order holds the order model and its lifecycle.
package order

import (
	"crypto/rand"
	"encoding/hex"
	"time"
)

// State is where an order is in its lifecycle.
type State string

const (
	StateCreated       State = "created"
	StatePaid          State = "paid"
	StatePaymentFailed State = "payment_failed"
	StateFulfilling    State = "fulfilling"
	StateShipped       State = "shipped"
	StateDelivered     State = "delivered"
	StateCancelled     State = "cancelled"
	StateOnHold        State = "on_hold"
)

// Item is one line of an order.
type Item struct {
	SKU        string `json:"sku"`
	Quantity   int    `json:"quantity"`
	PriceCents int64  `json:"price_cents"`
}

// Order is a customer order.
type Order struct {
	ID             string    `json:"id"`
	CustomerID     string    `json:"customer_id"`
	Items          []Item    `json:"items"`
	TotalCents     int64     `json:"total_cents"`
	Currency       string    `json:"currency"`
	State          State     `json:"state"`
	TrackingNumber string    `json:"tracking_number,omitempty"`
	CreatedAt      time.Time `json:"created_at"`
	UpdatedAt      time.Time `json:"updated_at"`
}

// New returns an order in its initial state, created.
func New(customerID, currency string, items []Item) *Order {
	var total int64
	for _, it := range items {
		total += it.PriceCents * int64(it.Quantity)
	}
	now := time.Now().UTC()
	return &Order{
		ID:         newID(),
		CustomerID: customerID,
		Items:      items,
		TotalCents: total,
		Currency:   currency,
		State:      StateCreated,
		CreatedAt:  now,
		UpdatedAt:  now,
	}
}

// newID returns an ID like ord_1f2e3d4c5b6a7980, the shape @acme/kit checks.
func newID() string {
	b := make([]byte, 8)
	_, _ = rand.Read(b)
	return "ord_" + hex.EncodeToString(b)
}
