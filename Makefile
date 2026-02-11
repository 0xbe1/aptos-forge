.PHONY: build install release-patch release-minor release-major clean

BINARY := aptly
COMMIT_SHA := $(shell git rev-parse --short HEAD)
BUILD_DATE := $(shell date -u +%Y-%m-%dT%H:%M:%SZ)
CURRENT_VERSION := $(shell git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")

build:
	APTLY_VERSION=$(CURRENT_VERSION) \
	APTLY_GIT_SHA=$(COMMIT_SHA) \
	APTLY_BUILD_DATE=$(BUILD_DATE) \
	cargo build -p aptly-cli --release --bin $(BINARY)
	cp target/release/$(BINARY) ./$(BINARY)

install:
	cargo install --path crates/aptly-cli

release-patch:
	cargo release patch --no-publish --execute

release-minor:
	cargo release minor --no-publish --execute

release-major:
	cargo release major --no-publish --execute

clean:
	rm -f $(BINARY)
	cargo clean
