BINDIR ?= ./bin
ADC_BIN ?= adc

.PHONY: build
build:
	go build -o $(BINDIR)/$(ADC_BIN) main.go

### fmt:         Format all go codes
.PHONY: fmt
fmt:
	./utils/goimports-reviser.sh
