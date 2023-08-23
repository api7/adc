#/bin/bash

default: help
help:  ## Display this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
.PHONY: help

build: ## Build adc
	@go build -o bin/adc main.go
.PHONY: build

test: ## Run cli test
	@cd test/cli && ginkgo -r
.PHONY: test

unit-test: ## Run unit test
	@go test -v ./...
.PHONY: unit-test

.PHONY: fmt
fmt: ## Format all go codes
	./utils/goimports-reviser.sh


