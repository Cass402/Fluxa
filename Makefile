.PHONY: up down build test deploy clean setup-test-accounts dev-shell lint format check init-hooks upgrade-deps coverage docs logs

# Start the development environment
up:
	docker compose up -d

# Stop the development environment
down:
	docker compose down

# Rebuild the Docker images
build:
	docker compose build

# Run tests
test:
	docker compose exec fluxa-dev bash -c "cd /app && anchor test"

# Deploy
deploy:
	docker compose exec fluxa-dev bash -c "cd /app && ./scripts/deploy.sh"

# Clean build artifacts
clean:
	docker compose exec fluxa-dev bash -c "cd /app/programs && cargo clean"
	docker compose exec fluxa-dev bash -c "cd /app/frontend && rm -rf .next"

# Generate test accounts and airdrop SOL
setup-test-accounts:
	docker compose exec fluxa-dev bash -c "cd /app && ./scripts/setup_test_accounts.sh"

# Enter development shell
dev-shell:
	docker compose exec fluxa-dev bash

# Run linting on all code
lint:
	docker compose exec fluxa-dev bash -c "cd /app/programs && cargo clippy -- -D warnings"
	docker compose exec fluxa-dev bash -c "cd /app/frontend && npm run lint"

# Format all code
format:
	docker compose exec fluxa-dev bash -c "cd /app/programs && cargo fmt --all"
	docker compose exec fluxa-dev bash -c "cd /app/frontend && npm run format"

# Run all checks (format + lint)
check:
	docker compose exec fluxa-dev bash -c "cd /app/programs && cargo fmt --all -- --check && cargo clippy -- -D warnings"
	docker compose exec fluxa-dev bash -c "cd /app/frontend && npm run lint"

# Initialize git hooks
init-hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit
	@echo "Git hooks initialized!"

# Upgrade dependencies
upgrade-deps:
	docker compose exec fluxa-dev bash -c "cd /app/programs && cargo update"
	docker compose exec fluxa-dev bash -c "cd /app/frontend && npm update"

# Generate code coverage report
coverage:
	docker compose exec fluxa-dev bash -c "cd /app && RUSTFLAGS=\"-C instrument-coverage\" LLVM_PROFILE_FILE=\"fluxa-%p-%m.profraw\" cargo test && grcov . -s /app --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./coverage/"
	@echo "Coverage report generated in ./coverage/index.html"

# Generate documentation
docs:
	docker compose exec fluxa-dev bash -c "cd /app/programs && cargo doc --no-deps --document-private-items"
	@echo "Documentation generated in ./target/doc/"

# View logs of running services
logs:
	docker-compose logs -f