.PHONY: help
help:
	@echo "Available make commands:"
	@cat Makefile | grep '^[a-z][^:]*:' | cut -d: -f1 | sort | sed 's/^/  /'

.PHONY: typeshare
typeshare:
    #typeshare --lang typescript --output-dir ./sf-viz/generated-types ./src

.PHONY: build-server
build-server:
	docker build -t sfromens/sf-server -f sf-server/Dockerfile .

.PHONY: run-server
run-server:
	docker run -p 8080:8080 sfromens/sf-server
