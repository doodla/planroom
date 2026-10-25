// Package http serves the orders API described in api/openapi.yaml.
package http

import (
	"encoding/json"
	"errors"
	"log/slog"
	"net/http"

	"github.com/acme-shop/orders/internal/order"
	"github.com/acme-shop/orders/internal/store"
)

// Handler serves the orders API.
type Handler struct {
	store *store.Store
	log   *slog.Logger
}

// New returns a Handler backed by st.
func New(st *store.Store, log *slog.Logger) *Handler {
	return &Handler{store: st, log: log}
}

// Routes registers every endpoint in api/openapi.yaml.
func (h *Handler) Routes() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("POST /orders", h.createOrder)
	mux.HandleFunc("GET /orders/{id}", h.getOrder)
	mux.HandleFunc("GET /healthz", h.healthz)
	return mux
}

type createOrderRequest struct {
	CustomerID string       `json:"customer_id"`
	Currency   string       `json:"currency"`
	Items      []order.Item `json:"items"`
	PaymentRef string       `json:"payment_ref"`
}

func (h *Handler) createOrder(w http.ResponseWriter, r *http.Request) {
	var req createOrderRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "invalid JSON body", http.StatusBadRequest)
		return
	}
	if req.CustomerID == "" || len(req.Items) == 0 {
		http.Error(w, "customer_id and items are required", http.StatusUnprocessableEntity)
		return
	}
	o := order.New(req.CustomerID, req.Currency, req.Items)
	if req.PaymentRef != "" {
		// Checkout charges the card before calling us, so the order starts paid.
		if _, err := order.Apply(o, order.EventPay); err != nil {
			http.Error(w, err.Error(), http.StatusConflict)
			return
		}
	}
	if err := h.store.Create(r.Context(), o); err != nil {
		h.log.Error("create order", "err", err)
		http.Error(w, "could not save order", http.StatusInternalServerError)
		return
	}
	writeJSON(w, http.StatusCreated, o)
}

func (h *Handler) getOrder(w http.ResponseWriter, r *http.Request) {
	o, ok := h.load(w, r)
	if !ok {
		return
	}
	writeJSON(w, http.StatusOK, o)
}

func (h *Handler) healthz(w http.ResponseWriter, r *http.Request) {
	if err := h.store.Ping(r.Context()); err != nil {
		http.Error(w, "database unreachable", http.StatusServiceUnavailable)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// load fetches the order named by the {id} path segment, or writes the error.
func (h *Handler) load(w http.ResponseWriter, r *http.Request) (*order.Order, bool) {
	o, err := h.store.Get(r.Context(), r.PathValue("id"))
	if errors.Is(err, store.ErrNotFound) {
		http.Error(w, "order not found", http.StatusNotFound)
		return nil, false
	}
	if err != nil {
		h.log.Error("load order", "err", err)
		http.Error(w, "could not load order", http.StatusInternalServerError)
		return nil, false
	}
	return o, true
}

func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}
