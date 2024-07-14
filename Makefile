default: help
help:  ## Display this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
.PHONY: help

run-api7:
	@docker login hkccr.ccs.tencentyun.com -u 100033089146 -p "{e>rw2[#EDAD" ## read only account
	@docker compose -f ./e2e/api7/docker-compose.yaml up -d
	@./e2e/api7/init.sh
.PHONY: run-api7
