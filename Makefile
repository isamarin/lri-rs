.PHONY: build release release-fast install bench check lumen lumen-release version version-bump version-check

version:
	@./scripts/calver show

version-bump:
	@./scripts/calver bump

version-check:
	@./scripts/calver check

build:
	cargo build -p light

release:
	cargo build --release -p light

lumen:
	cargo build -p lumen

lumen-release:
	cargo build --release -p lumen

release-fast:
	cargo build --profile release-fast -p light

install:
	cargo install --path light --force

bench:
	cargo bench -p lri-rs

check:
	cargo check --workspace