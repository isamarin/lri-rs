.PHONY: build release release-fast install bench check

build:
	cargo build -p light

release:
	cargo build --release -p light

release-fast:
	cargo build --profile release-fast -p light

install:
	cargo install --path light --force

bench:
	cargo bench -p lri-rs

check:
	cargo check --workspace