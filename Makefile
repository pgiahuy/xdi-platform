# Makefile - lối vào duy nhất cho build, test, chạy. CI gọi đúng các target này.
# Mọi target đều .PHONY; mỗi target vài dòng gọi cargo, npm, docker.

SHELL := /bin/sh
PREFIX ?= /usr/local
COMPOSE ?= docker compose

.PHONY: help dev dev-frontend build test check check-fast check-frontend up down logs openapi licenses hooks install uninstall release-artifacts clean

help: ## In danh sách target
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-12s %s\n", $$1, $$2}'

dev: ## Postgres trong Docker, backend chạy local với cargo run
	$(COMPOSE) up -d postgres
	cd backend && cargo run

dev-frontend: ## Giao diện chạy dev ở cổng 5173; /api chuyển sang backend
	cd frontend && npm install && npm run dev

build: ## Build backend ở chế độ release
	cd backend && cargo build --release

test: ## Test backend
	cd backend && cargo test

check-fast: ## fmt, clippy, test backend; không build docker và frontend
	cd backend && cargo fmt --check
	cd backend && cargo clippy --all-targets -- -D warnings
	cd backend && cargo test

check-frontend: ## Frontend: cài đúng theo package-lock, lint, schema.ts khớp openapi.json, kiểm kiểu, build
	cd frontend && npm ci && npm run lint && npm run api:check && npm run build

check: check-fast check-frontend ## check-fast, check-frontend và build docker
	$(COMPOSE) build

up: ## Chạy toàn bộ bằng docker compose
	$(COMPOSE) up --build

down: ## Dừng và giữ dữ liệu
	$(COMPOSE) down

logs: ## Xem log
	$(COMPOSE) logs -f

openapi: ## Sinh openapi.json từ code backend rồi sinh kiểu TypeScript frontend/src/api/schema.ts; commit cả hai
	@# Lệnh openapi của backend không kết nối database, chạy được khi chưa có Postgres. Ghi ra file tạm rồi mới đổi tên:
	@# shell mở và cắt rỗng file đích trước khi cargo chạy, nên code lỗi biên dịch sẽ để lại openapi.json rỗng.
	cd backend && cargo run --quiet -- openapi > ../openapi.json.tmp && mv ../openapi.json.tmp ../openapi.json
	cd frontend && npm run --silent api:types

licenses: ## Sinh docs/licenses-dependencies.md từ Cargo.lock và package-lock.json
	node scripts/licenses.mjs

hooks: ## Cài hook pre-push: check-fast, thêm check-frontend khi nhánh đổi frontend/, openapi.json hoặc Makefile
	@# git rev-parse tìm đúng thư mục hook, kể cả khi đang ở một git worktree (.git là file, không phải thư mục).
	install -m 755 scripts/pre-push "$$(git rev-parse --git-path hooks)/pre-push"
	@echo "đã cài hook pre-push; chạy lại make hooks khi scripts/pre-push thay đổi"

install: build ## Cài binary backend vào $(PREFIX)/bin (mặc định /usr/local; đổi bằng PREFIX=...)
	@test -n "$(PREFIX)" || { echo "PREFIX is empty" >&2; exit 1; }
	install -d "$(DESTDIR)$(PREFIX)/bin"
	install -m 755 backend/target/release/xdi-backend "$(DESTDIR)$(PREFIX)/bin/xdi-backend"

uninstall: ## Gỡ binary đã cài bằng make install
	@# PREFIX= rỗng sẽ biến lệnh dưới thành rm -f /bin/xdi-backend.
	@test -n "$(PREFIX)" || { echo "PREFIX is empty" >&2; exit 1; }
	rm -f "$(DESTDIR)$(PREFIX)/bin/xdi-backend"

# Gói phát hành, CI gọi đúng target này. Ra thư mục release/:
#   xdi-platform-$(VERSION).tar.gz                 mã nguồn, giải nén ra xdi-platform-$(VERSION)/
#   xdi-backend-$(VERSION)-linux-x86_64.tar.gz     binary kèm LICENSE, README, LICENSES.md và danh sách giấy phép phụ thuộc
#   notes.md                                        ghi chú lấy từ CHANGELOG.md
# DRY_RUN=1: chạy thử, chấp nhận CHANGELOG chưa có mục cho phiên bản này (CI dùng trên PR).
release-artifacts: ## Đóng gói phát hành: make release-artifacts VERSION=x.y.z
	@test -n "$(VERSION)" || { echo "VERSION is required, for example make release-artifacts VERSION=0.1.0"; exit 1; }
	scripts/check-version.sh $(VERSION)
	@scripts/release-notes.sh $(VERSION) > /dev/null || test "$(DRY_RUN)" = 1
	cd backend && cargo build --release --locked
	rm -rf release && mkdir -p release/xdi-backend-$(VERSION)-linux-x86_64
	cp backend/target/release/xdi-backend LICENSE README.md docs/LICENSES.md docs/licenses-dependencies.md \
		backend/third-party/swagger-ui-5.32.6-licenses.txt release/xdi-backend-$(VERSION)-linux-x86_64/
	tar -C release -czf release/xdi-backend-$(VERSION)-linux-x86_64.tar.gz xdi-backend-$(VERSION)-linux-x86_64
	rm -rf release/xdi-backend-$(VERSION)-linux-x86_64
	git archive --format=tar.gz --prefix=xdi-platform-$(VERSION)/ -o release/xdi-platform-$(VERSION).tar.gz HEAD
	scripts/release-notes.sh $(VERSION) > release/notes.md || { test "$(DRY_RUN)" = 1 && echo "Dry run: CHANGELOG.md has no section for $(VERSION)." > release/notes.md; }

clean: ## Xóa sản phẩm build
	cd backend && cargo clean
