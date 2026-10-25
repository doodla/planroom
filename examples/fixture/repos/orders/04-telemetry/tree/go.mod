module github.com/acme-shop/orders

go 1.22

require (
	github.com/jackc/pgx/v5 v5.6.0
	github.com/prometheus/client_golang v1.20.4
	github.com/redis/go-redis/v9 v9.6.1
	go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp v0.55.0
	go.opentelemetry.io/otel v1.30.0
	go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracegrpc v1.30.0
	go.opentelemetry.io/otel/exporters/prometheus v0.52.0
	go.opentelemetry.io/otel/metric v1.30.0
	go.opentelemetry.io/otel/sdk v1.30.0
	go.opentelemetry.io/otel/sdk/metric v1.30.0
)
