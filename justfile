test: test-tokio-uring test-monoio

test-tokio-uring:
    cargo test --no-default-features --features=macros,tokio-uring

test-monoio:
    cargo test --no-default-features --features=macros,monoio

lint: lint-tokio-uring lint-monoio

lint-tokio-uring:
    cargo clippy --no-default-features --features=macros,tokio-uring --workspace

lint-monoio:
    cargo clippy --no-default-features --features=macros,monoio --workspace

check-formatting:
    cargo fmt --check --all

fix-formatting:
    cargo fmt --all
