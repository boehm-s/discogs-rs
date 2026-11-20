# Load .env file
set dotenv-load

default:
    @just --list

# Build the crate
build *ARGS:
    cargo build {{ ARGS }}

# Check the code (including tests)
check:
    cargo check --tests --features xml-exports

# Run unit tests and doctests
test *ARGS:
    cargo nextest run --features xml-exports --failure-output final --success-output never {{ ARGS }}
    cargo test --doc --features xml-exports

# Run the tests against the real Discogs API (needs DISCOGS_TOKEN in .env)
test-live *ARGS:
    cargo nextest run --features live-tests,xml-exports -E 'test(live_tests)' {{ ARGS }}

# Format the code
fmt:
    cargo fmt --all

# Run every check done by the CI lint job
lint:
    cargo fmt --all -- --check
    cargo clippy --no-deps --all-targets --features xml-exports -- -D warnings
    cargo deny check --hide-inclusion-graph licenses sources advisories bans
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features xml-exports

# Check the public API against the latest version published on crates.io
semver:
    cargo semver-checks check-release

# Build and open the documentation
doc:
    cargo doc --no-deps --features xml-exports --open

# Check that the crate can be packaged for crates.io
package:
    cargo package

# Run everything the CI runs on a pull request
ci: lint test package
