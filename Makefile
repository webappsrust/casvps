.PHONY: build test run dev clean shell db-init

# Docker-based development
build:
	docker-compose build

dev:
	docker-compose up dev

test:
	docker-compose run --rm dev cargo test

run:
	docker-compose up test

shell:
	docker-compose run --rm shell

clean:
	docker-compose down -v
	rm -rf target/

# Database initialization
db-init:
	docker-compose run --rm dev cargo run -- init

# Quick build in container
quick-build:
	docker-compose run --rm dev cargo build

# Format code
fmt:
	docker-compose run --rm dev cargo fmt

# Lint
lint:
	docker-compose run --rm dev cargo clippy -- -D warnings

# Full check
check: fmt lint test
	@echo "All checks passed!"