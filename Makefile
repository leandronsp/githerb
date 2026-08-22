.DEFAULT_GOAL := help

BIN := target/release/githerb

.PHONY: help build install run test check fmt lint smoke clean

help: ## List the targets
	@grep -E '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

build: ## Build the release binary into target/release/
	@cargo build --release

install: ## Install the binary onto the PATH
	@cargo install --path . --locked

run: ## Build and run, ARGS="..." to pass arguments
	@cargo run --quiet -- $(ARGS)

test: ## Run the suite
	@cargo test --workspace --quiet

check: fmt lint test ## The gate: format, lint with warnings as errors, tests

fmt: ## Fail if anything is unformatted
	@cargo fmt --all --check

lint: ## clippy, warnings as errors
	@cargo clippy --workspace --all-targets -- -D warnings

smoke: build ## End to end in a browser: propose, annotate, watch it move, land
	@bin/smoke.sh

clean: ## Remove build output
	@cargo clean
