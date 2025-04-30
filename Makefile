.PHONY: help
help:
	@echo "Available make commands:"
	@cat Makefile | grep '^[a-z][^:]*:' | cut -d: -f1 | sort | sed 's/^/  /'

.PHONY: typeshare
typeshare:
    #typeshare --lang typescript --output-dir ./sf-viz/generated-types ./src
