.DEFAULT_GOAL := help

BIN     := bin/githerb
PKG     := ./...
GOBIN   := $(shell go env GOPATH)/bin
LINT    := $(GOBIN)/golangci-lint
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)
LDFLAGS := -s -w -X main.version=$(VERSION)

.PHONY: help build install run test cover check lint fmt vet tidy tools smoke clean

help: ## List the targets
	@grep -E '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

build: ## Build the binary into bin/
	@mkdir -p bin
	go build -trimpath -ldflags '$(LDFLAGS)' -o $(BIN) ./cmd/githerb

install: ## Install the binary onto the PATH
	go install -trimpath -ldflags '$(LDFLAGS)' ./cmd/githerb

run: build ## Build and run, ARGS="..." to pass arguments
	@$(BIN) $(ARGS)

test: ## Run the suite
	go test -race -shuffle=on $(PKG)

cover: ## Run the suite and open the coverage report
	go test -race -coverprofile=coverage.out -covermode=atomic $(PKG)
	go tool cover -func=coverage.out | tail -1

check: fmt vet lint test ## The gate: format, vet, lint, tests

fmt: ## Fail if anything is unformatted
	@test -z "$$(gofmt -l . | tee /dev/stderr)" || { echo "run gofmt -w ."; exit 1; }

vet: ## go vet
	go vet $(PKG)

lint: $(LINT) ## golangci-lint
	@$(LINT) run

tidy: ## Tidy and verify the module
	go mod tidy
	go mod verify

tools: $(LINT) ## Install the tools the gate needs

$(LINT):
	go install github.com/golangci/golangci-lint/v2/cmd/golangci-lint@latest

smoke: build ## End to end in a browser: propose, annotate, watch it move, land
	@bin/smoke.sh

clean: ## Remove build output
	rm -rf bin dist coverage.out
