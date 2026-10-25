// Command orders runs the orders HTTP API.
package main

import (
	"context"
	"errors"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/acme-shop/orders/internal/config"
	ordershttp "github.com/acme-shop/orders/internal/http"
	"github.com/acme-shop/orders/internal/queue"
	"github.com/acme-shop/orders/internal/store"
	"github.com/acme-shop/orders/internal/telemetry"
)

func main() {
	log := slog.New(slog.NewJSONHandler(os.Stdout, nil))
	if err := run(log); err != nil {
		log.Error("orders exited", "err", err)
		os.Exit(1)
	}
}

func run(log *slog.Logger) error {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	cfg, err := config.Load()
	if err != nil {
		return err
	}

	shutdownTelemetry, err := telemetry.Setup(ctx, cfg.MetricsAddr)
	if err != nil {
		return err
	}

	st, err := store.Open(ctx, cfg.DatabaseURL)
	if err != nil {
		return err
	}
	defer st.Close()

	jobs, err := queue.NewPublisher(cfg.RedisURL)
	if err != nil {
		return err
	}
	defer jobs.Close()

	h := ordershttp.New(st, jobs, log)
	srv := &http.Server{
		Addr:              cfg.Addr,
		Handler:           ordershttp.Instrument(h.Routes()),
		ReadHeaderTimeout: 5 * time.Second,
	}
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = srv.Shutdown(shutdownCtx)
		_ = shutdownTelemetry(shutdownCtx)
	}()

	log.Info("orders listening", "addr", cfg.Addr, "metrics", cfg.MetricsAddr)
	if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}
