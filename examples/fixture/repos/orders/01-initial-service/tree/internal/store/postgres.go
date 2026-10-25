// Package store persists orders in Postgres (orders-db).
package store

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/acme-shop/orders/internal/order"
)

// ErrNotFound is returned when no order has the given ID.
var ErrNotFound = errors.New("store: order not found")

// Store reads and writes orders.
type Store struct {
	pool *pgxpool.Pool
}

// Open connects to the database at url (DATABASE_URL).
func Open(ctx context.Context, url string) (*Store, error) {
	pool, err := pgxpool.New(ctx, url)
	if err != nil {
		return nil, fmt.Errorf("store: connect: %w", err)
	}
	if err := pool.Ping(ctx); err != nil {
		pool.Close()
		return nil, fmt.Errorf("store: ping: %w", err)
	}
	return &Store{pool: pool}, nil
}

// Close releases the pool.
func (s *Store) Close() { s.pool.Close() }

// Ping checks that the database is reachable.
func (s *Store) Ping(ctx context.Context) error { return s.pool.Ping(ctx) }

// Create inserts a new order.
func (s *Store) Create(ctx context.Context, o *order.Order) error {
	items, err := json.Marshal(o.Items)
	if err != nil {
		return err
	}
	_, err = s.pool.Exec(ctx,
		`INSERT INTO orders (id, customer_id, items, total_cents, currency, state, created_at, updated_at)
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`,
		o.ID, o.CustomerID, items, o.TotalCents, o.Currency, o.State, o.CreatedAt, o.UpdatedAt)
	return err
}

// Get loads one order.
func (s *Store) Get(ctx context.Context, id string) (*order.Order, error) {
	var o order.Order
	var items []byte
	err := s.pool.QueryRow(ctx,
		`SELECT id, customer_id, items, total_cents, currency, state, created_at, updated_at
		 FROM orders WHERE id = $1`, id).
		Scan(&o.ID, &o.CustomerID, &items, &o.TotalCents, &o.Currency, &o.State, &o.CreatedAt, &o.UpdatedAt)
	if errors.Is(err, pgx.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, err
	}
	if err := json.Unmarshal(items, &o.Items); err != nil {
		return nil, err
	}
	return &o, nil
}

// Transition saves o's new state and appends a row to its event log.
func (s *Store) Transition(ctx context.Context, o *order.Order, from order.State, detail string) error {
	tx, err := s.pool.Begin(ctx)
	if err != nil {
		return err
	}
	defer tx.Rollback(ctx)
	if _, err := tx.Exec(ctx,
		`UPDATE orders SET state = $2, updated_at = $3 WHERE id = $1`,
		o.ID, o.State, o.UpdatedAt); err != nil {
		return err
	}
	if _, err := tx.Exec(ctx,
		`INSERT INTO order_events (order_id, from_state, to_state, detail, at) VALUES ($1, $2, $3, $4, $5)`,
		o.ID, from, o.State, detail, o.UpdatedAt); err != nil {
		return err
	}
	return tx.Commit(ctx)
}
