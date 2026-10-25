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
}

// Load reads the environment. DATABASE_URL is required.
func Load() (Config, error) {
	c := Config{
		Addr:        getenv("ORDERS_ADDR", ":8080"),
		DatabaseURL: os.Getenv("DATABASE_URL"),
	}
	if c.DatabaseURL == "" {
		return c, fmt.Errorf("config: DATABASE_URL is required")
	}
	return c, nil
}

func getenv(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}
