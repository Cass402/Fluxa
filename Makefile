.PHONY: up down build test deploy clean

# Start the development environment
up:
	docker-compose up -d
# Stop the development environment
down:
	docker-compose down
# Rebuild the Docker images
build:
	docker-compose build
# Run tests
test:
	docker-compose exec fluxa-dev bash -c "cd /app && anchor test"
# Deploy
deploy:
	docker-compose exec fluxa-dev bash -c "cd /app && ./scripts/deploy.sh"
# Clean build artifacts
clean:
	docker-compose exec fluxa-dev bash -c "cd /app/programs && cargo clean"
	docker-compose exec fluxa-dev bash -c "cd /app/frontend && rm -rf .next"
# Generate test accounts and airdrop SOL
setup-test-accounts:
	docker-compose exec fluxa-dev bash -c "cd /app && ./scripts/setup_test_accounts.sh"
# Enter development shell
dev-shell:
	docker-compose exec fluxa-dev bash