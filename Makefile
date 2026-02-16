.PHONY: build install release clean \
	release-patch release-minor release-major

BINARY := aptly
COMMIT_SHA := $(shell git rev-parse --short HEAD)
BUILD_DATE := $(shell date -u +%Y-%m-%dT%H:%M:%SZ)
CURRENT_VERSION := $(shell (git describe --tags --match "aptly-cli-v*" --abbrev=0 2>/dev/null || echo "aptly-cli-v0.0.0") | sed 's/^aptly-cli-//')

build:
	APTLY_VERSION=$(CURRENT_VERSION) \
	APTLY_GIT_SHA=$(COMMIT_SHA) \
	APTLY_BUILD_DATE=$(BUILD_DATE) \
	cargo build -p aptly-cli --release --bin $(BINARY)
	cp target/release/$(BINARY) ./$(BINARY)

install:
	APTLY_VERSION=$(CURRENT_VERSION) \
	APTLY_GIT_SHA=$(COMMIT_SHA) \
	APTLY_BUILD_DATE=$(BUILD_DATE) \
	cargo build -p aptly-cli --release --bin $(BINARY)
	install -m 755 target/release/$(BINARY) /usr/local/bin/$(BINARY)

release:
	@echo "Local release targets:"
	@echo "  make release-patch TARGET=cli|aptos|compose"
	@echo "  make release-minor TARGET=cli|aptos|compose"
	@echo "  make release-major TARGET=cli|aptos|compose"

release-patch:
	@case "$(TARGET)" in \
		cli) PACKAGE="aptly-cli"; TAG="aptly-cli-v{{version}}" ;; \
		aptos) PACKAGE="aptly-aptos"; TAG="aptly-aptos-v{{version}}" ;; \
		compose) PACKAGE="aptos-script-compose"; TAG="aptos-script-compose-v{{version}}" ;; \
		*) echo "usage: make $@ TARGET=cli|aptos|compose"; exit 1 ;; \
	esac; \
	cargo release -p "$$PACKAGE" patch --no-publish --execute --tag-name "$$TAG"

release-minor:
	@case "$(TARGET)" in \
		cli) PACKAGE="aptly-cli"; TAG="aptly-cli-v{{version}}" ;; \
		aptos) PACKAGE="aptly-aptos"; TAG="aptly-aptos-v{{version}}" ;; \
		compose) PACKAGE="aptos-script-compose"; TAG="aptos-script-compose-v{{version}}" ;; \
		*) echo "usage: make $@ TARGET=cli|aptos|compose"; exit 1 ;; \
	esac; \
	cargo release -p "$$PACKAGE" minor --no-publish --execute --tag-name "$$TAG"

release-major:
	@case "$(TARGET)" in \
		cli) PACKAGE="aptly-cli"; TAG="aptly-cli-v{{version}}" ;; \
		aptos) PACKAGE="aptly-aptos"; TAG="aptly-aptos-v{{version}}" ;; \
		compose) PACKAGE="aptos-script-compose"; TAG="aptos-script-compose-v{{version}}" ;; \
		*) echo "usage: make $@ TARGET=cli|aptos|compose"; exit 1 ;; \
	esac; \
	cargo release -p "$$PACKAGE" major --no-publish --execute --tag-name "$$TAG"

clean:
	rm -f $(BINARY)
	cargo clean
