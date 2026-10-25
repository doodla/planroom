package http

import (
	"net/http"
	"strconv"
	"time"

	"go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/metric"
)

type statusRecorder struct {
	http.ResponseWriter
	status int
}

func (r *statusRecorder) WriteHeader(code int) {
	r.status = code
	r.ResponseWriter.WriteHeader(code)
}

// Instrument wraps next with tracing and the request metrics that the
// OrdersHighErrorRate alert and the orders-overview dashboard read. The
// Prometheus exporter publishes them as orders_http_requests_total and
// orders_http_request_duration_seconds.
func Instrument(next http.Handler) http.Handler {
	meter := otel.Meter("github.com/acme-shop/orders/internal/http")
	requests, _ := meter.Int64Counter("orders_http_requests",
		metric.WithDescription("HTTP requests served, by method and status code."))
	duration, _ := meter.Float64Histogram("orders_http_request_duration",
		metric.WithUnit("s"),
		metric.WithDescription("HTTP request latency."))

	h := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		rec := &statusRecorder{ResponseWriter: w, status: http.StatusOK}
		next.ServeHTTP(rec, r)
		attrs := metric.WithAttributes(
			attribute.String("method", r.Method),
			attribute.String("code", strconv.Itoa(rec.status)),
		)
		requests.Add(r.Context(), 1, attrs)
		duration.Record(r.Context(), time.Since(start).Seconds(), attrs)
	})
	return otelhttp.NewHandler(h, "orders")
}
