// Package http serves the orders API described in api/openapi.yaml.
package http

import (
	"context"
	"encoding/json"
	"errors"
	"log/slog"
	"net/http"
	"regexp"

	"github.com/acme-shop/orders/internal/order"
	"github.com/acme-shop/orders/internal/queue"
	"github.com/acme-shop/orders/internal/store"
)

// JobPublisher enqueues fulfilment work for the worker.
type JobPublisher interface {
	Publish(ctx context.Context, j queue.Job) error
}

// Handler serves the orders API.
type Handler struct {
	store *store.Store
	jobs  JobPublisher
	log   *slog.Logger
}

// New returns a Handler backed by st that hands paid orders to jobs.
func New(st *store.Store, jobs JobPublisher, log *slog.Logger) *Handler {
	return &Handler{store: st, jobs: jobs, log: log}
}

// Routes registers every endpoint in api/openapi.yaml.
func (h *Handler) Routes() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("POST /orders", h.createOrder)
	mux.HandleFunc("GET /orders/{id}", h.getOrder)
	mux.HandleFunc("POST /orders/{id}/events", h.applyEvent)
	mux.HandleFunc("GET /healthz", h.healthz)
	return mux
}

// currencyCode is an ISO 4217 alphabetic code.
var currencyCode = regexp.MustCompile(`^[A-Z]{3}$`)

type createOrderRequest struct {
	CustomerID string       `json:"customer_id"`
	Currency   string       `json:"currency"`
	Items      []order.Item `json:"items"`
	PaymentRef string       `json:"payment_ref"`
}

func (h *Handler) createOrder(w http.ResponseWriter, r *http.Request) {
	var req createOrderRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeProblem(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.CustomerID == "" || len(req.Items) == 0 {
		writeProblem(w, http.StatusUnprocessableEntity, "customer_id and items are required")
		return
	}
	if req.Currency == "" {
		req.Currency = "USD"
	}
	if !currencyCode.MatchString(req.Currency) {
		writeProblem(w, http.StatusUnprocessableEntity, "currency must be an ISO 4217 code such as USD")
		return
	}
	o := order.New(req.CustomerID, req.Currency, req.Items)
	if req.PaymentRef != "" {
		// Checkout charges the card before calling us, so the order starts paid.
		if _, err := order.Apply(o, order.EventPay); err != nil {
			writeProblem(w, http.StatusConflict, err.Error())
			return
		}
	}
	if err := h.store.Create(r.Context(), o); err != nil {
		h.log.Error("create order", "err", err)
		writeProblem(w, http.StatusInternalServerError, "could not save order")
		return
	}
	h.enqueueFulfil(r.Context(), o)
	writeJSON(w, http.StatusCreated, o)
}

func (h *Handler) getOrder(w http.ResponseWriter, r *http.Request) {
	o, ok := h.load(w, r)
	if !ok {
		return
	}
	writeJSON(w, http.StatusOK, o)
}

type eventRequest struct {
	Event          order.Event `json:"event"`
	TrackingNumber string      `json:"tracking_number,omitempty"`
}

// applyEvent moves an order through one lifecycle event (pay, fulfil, ship, ...).
func (h *Handler) applyEvent(w http.ResponseWriter, r *http.Request) {
	var req eventRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeProblem(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	o, ok := h.load(w, r)
	if !ok {
		return
	}
	if req.TrackingNumber != "" {
		o.TrackingNumber = req.TrackingNumber
	}
	from, err := order.Apply(o, req.Event)
	if errors.Is(err, order.ErrInvalidTransition) || errors.Is(err, order.ErrGuardFailed) {
		writeProblem(w, http.StatusConflict, err.Error())
		return
	}
	if err := h.store.Transition(r.Context(), o, from, req.TrackingNumber); err != nil {
		h.log.Error("save transition", "order", o.ID, "err", err)
		writeProblem(w, http.StatusInternalServerError, "could not save order")
		return
	}
	h.enqueueFulfil(r.Context(), o)
	writeJSON(w, http.StatusOK, o)
}

func (h *Handler) healthz(w http.ResponseWriter, r *http.Request) {
	if err := h.store.Ping(r.Context()); err != nil {
		writeProblem(w, http.StatusServiceUnavailable, "database unreachable")
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// load fetches the order named by the {id} path segment, or writes the error.
func (h *Handler) load(w http.ResponseWriter, r *http.Request) (*order.Order, bool) {
	o, err := h.store.Get(r.Context(), r.PathValue("id"))
	if errors.Is(err, store.ErrNotFound) {
		writeProblem(w, http.StatusNotFound, "order not found")
		return nil, false
	}
	if err != nil {
		h.log.Error("load order", "err", err)
		writeProblem(w, http.StatusInternalServerError, "could not load order")
		return nil, false
	}
	return o, true
}

// enqueueFulfil hands a paid order to the worker over the order-jobs queue.
func (h *Handler) enqueueFulfil(ctx context.Context, o *order.Order) {
	if o.State != order.StatePaid {
		return
	}
	if err := h.jobs.Publish(ctx, queue.Job{Kind: queue.KindFulfil, OrderID: o.ID}); err != nil {
		h.log.Error("publish fulfil job", "order", o.ID, "err", err)
	}
}
