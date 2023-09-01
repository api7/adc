#/bin/bash
PWD ?= $(shell pwd)
export PATH := $(PWD)/bin:$(PATH)

VERSION ?= dev
GITSHA = $(shell git rev-parse --short=7 HEAD)
LDFLAGS = "-X github.com/api7/adc/cmd.VERSION=$(VERSION) -X github.com/api7/adc/cmd.GitRevision=$(GITSHA)"

.DEFAULT_GOAL := help

.PHONY: help
help:  ## Display this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

.PHONY: build
build: ## Build adc
	@echo "Building adc..."
	@CGO_ENABLED=0 go build -o bin/adc -ldflags $(LDFLAGS) main.go

.PHONY: test
test: ## Run cli test
	@cd test/cli && ginkgo -r --nodes=1

.PHONY: unit-test
unit-test: ## Run unit test
	@go test -v $$(go list ./... | grep -v /test/)

.PHONY: fmt
fmt: ## Format all go codes
	./utils/goimports-reviser.sh
