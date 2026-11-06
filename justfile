# Load .env file (DISCOGS_TOKEN for live tests)
set dotenv-load

default:
    @just --list

# Build the crate
build *ARGS:
    cargo build --all-features {{ ARGS }}

# Check the code (including tests)
check:
    cargo check --all-features --tests

# Run unit tests and doctests
test *ARGS:
    cargo nextest run --all-features --failure-output final --success-output never {{ ARGS }}
    cargo test --all-features --doc

# Run the live tests against the real Discogs API (needs DISCOGS_TOKEN)
test-live *ARGS:
    cargo nextest run --all-features --run-ignored only --no-tests=pass {{ ARGS }}

# Format the code
fmt:
    cargo fmt --all

# Run every check done by the CI lint job
lint:
    cargo fmt --all -- --check
    cargo clippy --no-deps --all-features --tests -- -D warnings
    cargo deny check --hide-inclusion-graph licenses sources advisories bans
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Check the public API against the latest version published on crates.io
semver:
    cargo semver-checks check-release

# Build and open the documentation
doc:
    cargo doc --no-deps --all-features --open

# Check that the crate can be packaged for crates.io
package:
    cargo package

# Run everything the CI runs on a pull request
ci: lint test package
