// Package queue publishes fulfilment jobs to Redis for the worker.
package queue

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/redis/go-redis/v9"
)

// JobsKey is the Redis list the worker pops jobs from.
const JobsKey = "order-jobs"

// KindFulfil asks the worker to pick, pack and ship a paid order.
const KindFulfil = "fulfil"

// Job is one message on the order-jobs list.
type Job struct {
	Kind       string    `json:"kind"`
	OrderID    string    `json:"order_id"`
	EnqueuedAt time.Time `json:"enqueued_at"`
}

// Publisher pushes jobs onto JobsKey.
type Publisher struct {
	rdb *redis.Client
}

// NewPublisher connects to the Redis at url (REDIS_URL).
func NewPublisher(url string) (*Publisher, error) {
	opts, err := redis.ParseURL(url)
	if err != nil {
		return nil, fmt.Errorf("queue: parse REDIS_URL: %w", err)
	}
	return &Publisher{rdb: redis.NewClient(opts)}, nil
}

// Publish enqueues j. The worker pops with BRPOP, so LPUSH keeps jobs in order.
func (p *Publisher) Publish(ctx context.Context, j Job) error {
	if j.EnqueuedAt.IsZero() {
		j.EnqueuedAt = time.Now().UTC()
	}
	body, err := json.Marshal(j)
	if err != nil {
		return err
	}
	return p.rdb.LPush(ctx, JobsKey, body).Err()
}

// Close closes the Redis connection.
func (p *Publisher) Close() error { return p.rdb.Close() }
