# Makefile - lối vào duy nhất cho build, test, chạy. CI gọi đúng các target này.
# Mọi target đều .PHONY; mỗi target vài dòng gọi cargo, npm, docker.

SHELL := /bin/sh
PREFIX ?= /usr/local
COMPOSE ?= docker compose

.PHONY: help dev build test check check-fast up down logs openapi hooks install clean

help: ## In danh sách target
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-12s %s\n", $$1, $$2}'

dev: ## Postgres trong Docker, backend chạy local với cargo run
	$(COMPOSE) up -d postgres
	cd backend && cargo run

build: ## Build backend ở chế độ release
	cd backend && cargo build --release

test: ## Test backend
	cd backend && cargo test

check-fast: ## fmt, clippy, test backend; không build docker và frontend
	cd backend && cargo fmt --check
	cd backend && cargo clippy --all-targets -- -D warnings
	cd backend && cargo test

check: check-fast ## check-fast cộng build frontend (khi có) và build docker
	@if [ -f frontend/package.json ]; then cd frontend && npm ci && npm run lint && npm run build; fi
	$(COMPOSE) build

up: ## Chạy toàn bộ bằng docker compose
	$(COMPOSE) up --build

down: ## Dừng và giữ dữ liệu
	$(COMPOSE) down

logs: ## Xem log
	$(COMPOSE) logs -f

openapi: ## Sinh openapi.json từ backend (chưa có, thêm khi có utoipa)
	@echo "chưa có: thêm khi backend có utoipa"

hooks: ## Cài hook pre-push chạy check-fast
	printf '#!/bin/sh\nmake check-fast\n' > .git/hooks/pre-push && chmod +x .git/hooks/pre-push
	@echo "đã cài .git/hooks/pre-push"

install: build ## Cài binary backend vào $(PREFIX)/bin
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 backend/target/release/xdi-backend $(DESTDIR)$(PREFIX)/bin/xdi-backend

clean: ## Xóa sản phẩm build
	cd backend && cargo clean
