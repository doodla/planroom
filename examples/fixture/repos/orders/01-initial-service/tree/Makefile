.PHONY: build test lint run

build:
	CGO_ENABLED=0 go build -trimpath -o bin/orders ./cmd/orders

test:
	go test -race ./...

lint:
	golangci-lint run ./...

run: build
	DATABASE_URL=postgres://orders:orders@localhost:5432/orders?sslmode=disable ./bin/orders
