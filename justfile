_default:
    @just --list

# Build the project
build-debug:
    cargo build

# Build the project in release mode
build-release:
    cargo build --release

# Run checkers
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings
    cargo check

# Run formatter
format:
    cargo fmt --all

# Run tests
test:
    cargo test

# Run continuous integration
ci: check test
