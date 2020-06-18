fmt:
	cargo fmt --all -- --check

clippy:
	RUSTFLAGS='-F warnings' cargo clippy --all --tests

test:
	RUSTFLAGS='-F warnings' RUST_BACKTRACE=full cargo test --all

ci: fmt clippy test
	git diff --exit-code Cargo.lock

prod: ## Build binary with release profile.
	cargo build --release

integration:
	echo "TODO"

.PHONY: test clippy fmt ci prod
