build:
	cargo build --verbose --release --locked

# Run the rust tests
test:
	cargo nextest run --all-features --workspace --locked  --cargo-profile release --no-tests=warn

# Validate formatting
format:
	cargo +nightly fmt --check

# Errors if there is a warning with clippy
lint:
	cargo +nightly clippy --all-targets --workspace   --profile dev -- -D warnings

# Fix linting errors
lint-fix:
	cargo +nightly clippy --all-targets --workspace   --profile dev --fix 

# Can be used as a manual pre-commit check
pre-commit:
	cargo fmt && make lint

docker-build:
	docker build -t assertion-da:local .

docker-build-dev:
	docker build --build-arg BUILD_FLAGS="--features debug_assertions" -t assertion-da:dev-local .

# Run the docker image
compose:
	docker compose -f etc/docker-compose.yaml up

# Run the docker image for development
compose-dev:
	docker compose -f etc/docker-compose-dev.yaml up
