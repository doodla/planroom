// Package config reads the service's settings from the environment.
package config

import (
	"fmt"
	"os"
)

// Config is everything the service needs to start.
type Config struct {
	Addr        string
	DatabaseURL string
	RedisURL    string
	MetricsAddr string
}

// Load reads the environment. DATABASE_URL and REDIS_URL are required.
// OpenTelemetry reads its own OTEL_* variables.
func Load() (Config, error) {
	c := Config{
		Addr:        getenv("ORDERS_ADDR", ":8080"),
		DatabaseURL: os.Getenv("DATABASE_URL"),
		RedisURL:    os.Getenv("REDIS_URL"),
		MetricsAddr: getenv("METRICS_ADDR", ":9464"),
	}
	if c.DatabaseURL == "" {
		return c, fmt.Errorf("config: DATABASE_URL is required")
	}
	if c.RedisURL == "" {
		return c, fmt.Errorf("config: REDIS_URL is required")
	}
	return c, nil
}

func getenv(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}
