# Makefile - lối vào duy nhất cho build, test, chạy. CI gọi đúng các target này.
# Mọi target đều .PHONY; mỗi target vài dòng gọi cargo, npm, docker.

SHELL := /bin/sh
PREFIX ?= /usr/local
COMPOSE ?= docker compose
# Nơi make backup ghi bản sao lưu. Mặc định ngoài thư mục repo: bản sao lưu có dữ liệu và chuỗi băm mật khẩu.
BACKUP_DIR ?= $(HOME)/xdi-backups
# Binary do make build tạo ra. Cargo ghi vào CARGO_TARGET_DIR nếu biến này được đặt (thường gặp khi nhiều bản clone
# dùng chung một thư mục build), không thì vào backend/target; ghi cứng backend/target thì make install báo không có
# file. Đường dẫn tương đối trong CARGO_TARGET_DIR được cargo tính từ thư mục chạy lệnh, ở đây là backend/.
CARGO_TARGET := $(if $(filter /%,$(CARGO_TARGET_DIR)),$(CARGO_TARGET_DIR),backend/$(or $(CARGO_TARGET_DIR),target))
BACKEND_BIN := $(CARGO_TARGET)/release/xdi-backend
# Database mẫu nhỏ của compose (dịch vụ demo-db, data/demo-db/README.md), mở trên 127.0.0.1 cho backend chạy ngoài
# Docker (make dev, make eval). SOURCE_SECRET_KEY không đặt thì backend tự sinh khóa trong DATA_DIR.
DEMO_DB_PORT ?= 5434
SAMPLE_DATABASE_URL ?= postgres://analyst:analyst@127.0.0.1:$(DEMO_DB_PORT)/company_demo

.PHONY: help dev dev-frontend build test check check-fast check-frontend up down logs backup restore openapi licenses hooks install uninstall release-artifacts clean eval eval-classify

help: ## In danh sách target
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-12s %s\n", $$1, $$2}'

dev: ## Postgres và database mẫu trong Docker, backend chạy local với cargo run
	$(COMPOSE) up -d --wait postgres demo-db
	cd backend && SAMPLE_DATABASE_URL="$(SAMPLE_DATABASE_URL)" cargo run

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

# Sao lưu database trước, file dữ liệu sau, khi backend vẫn chạy. Hai bước không chụp cùng một thời điểm: ai đó nạp
# file, xóa dataset hay xóa workspace giữa pg_dump và tar thì database và file trong bản sao lưu có thể không khớp (ví
# dụ database còn workspace mà file của nó đã bị xóa). Cần chắc chắn thì dừng backend trước khi sao lưu
# (docker compose stop backend, xong thì docker compose up -d); không thì sao lưu lúc ít người dùng.
# Ghi vào thư mục .partial rồi mới đổi tên, để lỗi giữa chừng không để lại một bản trông như hoàn chỉnh. Đọc /data bằng
# run (container tạm) thay vì exec, để sao lưu được cả khi backend đang dừng hoặc khởi động lỗi. staging/ là file đang
# nạp dở, backend tự xóa khi khởi động.
backup: ## Sao lưu database và file dữ liệu vào ~/xdi-backups/<ngày-giờ>/ (đổi chỗ bằng BACKUP_DIR=...)
	@set -e; umask 077; \
	dir="$(BACKUP_DIR)/$$(date +%Y%m%d-%H%M%S)"; \
	test ! -e "$$dir" || { echo "$$dir already exists, run make backup again in a second" >&2; exit 1; }; \
	mkdir -p "$$dir.partial"; \
	$(COMPOSE) exec -T postgres pg_dump -U xdi -Fc xdi > "$$dir.partial/db.dump"; \
	$(COMPOSE) run --rm --no-deps -T backend tar czf - -C /data --exclude=./staging . > "$$dir.partial/data.tgz"; \
	mv "$$dir.partial" "$$dir"; \
	echo "backup written to $$dir"

# Thay toàn bộ dữ liệu hiện có bằng bản sao lưu. Trước khi dừng hay xóa gì, đọc hết cả hai file để chắc chúng không
# hỏng: file chép dở hay bị cắt cụt (ổ đầy) mà phát hiện sau khi đã xóa thì mất cả dữ liệu đang có. pg_restore -f
# /dev/null đọc toàn bộ db.dump mà không ghi vào database (pg_restore -l chỉ đọc mục lục, không bắt được file cắt cụt);
# gzip -t và tar -tzf đọc toàn bộ data.tgz.
# Sau đó dừng backend để không request nào ghi chen vào; xóa hẳn database rồi tạo lại thay vì pg_restore --clean, vì
# bảng của migration mới hơn bản sao lưu sẽ không bị --clean xóa và làm migration chạy lại bị lỗi. Khi khởi động lại,
# backend áp thêm các migration mới hơn bản sao lưu (nếu có).
restore: ## Khôi phục, THAY dữ liệu hiện có: make restore FROM=~/xdi-backups/<ngày-giờ>
	@test -n "$(FROM)" || { echo "FROM is required, for example make restore FROM=$(BACKUP_DIR)/20261022-101500" >&2; exit 1; }
	@test -f "$(FROM)/db.dump" && test -f "$(FROM)/data.tgz" || { echo "$(FROM) must contain db.dump and data.tgz" >&2; exit 1; }
	$(COMPOSE) up -d --wait postgres
	@$(COMPOSE) exec -T postgres pg_restore -f /dev/null < "$(FROM)/db.dump" || \
		{ echo "$(FROM)/db.dump is damaged or incomplete; nothing was changed" >&2; exit 1; }
	@gzip -t "$(FROM)/data.tgz" && tar -tzf "$(FROM)/data.tgz" > /dev/null || \
		{ echo "$(FROM)/data.tgz is damaged or incomplete; nothing was changed" >&2; exit 1; }
	@echo "backup files are readable, restoring"
	$(COMPOSE) stop backend
	$(COMPOSE) exec -T postgres dropdb -U xdi --if-exists --force xdi
	$(COMPOSE) exec -T postgres createdb -U xdi xdi
	$(COMPOSE) exec -T postgres pg_restore -U xdi -d xdi --exit-on-error < "$(FROM)/db.dump"
	@# 10001 là user không phải root mà backend chạy trong container (backend/Dockerfile).
	$(COMPOSE) run --rm --no-deps -T --user root backend \
		sh -c 'find /data -mindepth 1 -delete && tar xzf - -C /data && chown -R 10001 /data' < "$(FROM)/data.tgz"
	$(COMPOSE) up -d

eval: ## Bộ đánh giá câu hỏi mẫu (#37, #230): mock trên database mẫu (demo-db) và trên data/samples/dataco.csv, thoát mã 1 khi accuracy dưới eval/baseline.json
	@# Database riêng xdi_eval để không rác vào db dev; lệnh createdb bỏ qua lỗi khi compose chưa chạy (CI đặt sẵn DATABASE_URL).
	-$(COMPOSE) exec -T postgres createdb -U xdi xdi_eval 2>/dev/null
	$(COMPOSE) up -d --wait demo-db
	cd backend && cargo build -q
	@# Lần đầu: các bảng chụp từ database mẫu, như thiết lập lần đầu của bản cài mới. Lần hai: dataset DataCo từ file.
	DATABASE_URL=$(or $(DATABASE_URL),postgres://xdi:xdi@localhost:5433/xdi_eval) LLM_PROVIDER=mock DATA_DIR=backend/target/eval-data \
	  SAMPLES_DIR=data/samples SAMPLE_DATABASE_URL="$(SAMPLE_DATABASE_URL)" \
	  $(CARGO_TARGET)/debug/xdi-backend eval --sample-database --cases eval/sales_joins.json --cases eval/edu.json \
	  --baseline eval/baseline.json --out eval/result-database.json
	DATABASE_URL=$(or $(DATABASE_URL),postgres://xdi:xdi@localhost:5433/xdi_eval) LLM_PROVIDER=mock DATA_DIR=backend/target/eval-data \
	  $(CARGO_TARGET)/debug/xdi-backend eval --data data/samples/dataco.csv --cases eval/dataco.json --baseline eval/baseline.json

eval-classify: ## Bộ câu thử phân loại câu hỏi (#197): tỷ lệ đúng của luật từ khóa và của mô hình; mặc định mock, LLM_PROVIDER=gemini để đo thật
	-$(COMPOSE) exec -T postgres createdb -U xdi xdi_eval 2>/dev/null
	cd backend && cargo build -q
	DATABASE_URL=$(or $(DATABASE_URL),postgres://xdi:xdi@localhost:5433/xdi_eval) LLM_PROVIDER=$(or $(LLM_PROVIDER),mock) DATA_DIR=backend/target/eval-data \
	  $(CARGO_TARGET)/debug/xdi-backend eval-classify --data data/samples/dataco.csv --cases eval/classify.json $(if $(PAUSE_MS),--pause-ms $(PAUSE_MS))

openapi: ## Sinh openapi.json, response mẫu của ask và kiểu TypeScript frontend/src/api/schema.ts từ code; commit cả ba
	@# Lệnh openapi và ask-examples của backend không kết nối database, chạy được khi chưa có Postgres. Ghi ra file tạm
	@# rồi mới đổi tên: shell mở và cắt rỗng file đích trước khi cargo chạy, nên code lỗi biên dịch sẽ để lại file rỗng.
	cd backend && cargo run --quiet -- openapi > ../openapi.json.tmp && mv ../openapi.json.tmp ../openapi.json
	cd backend && cargo run --quiet -- ask-examples > ../frontend/src/api/examples/ask.json.tmp && \
		mv ../frontend/src/api/examples/ask.json.tmp ../frontend/src/api/examples/ask.json
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
	install -m 755 "$(BACKEND_BIN)" "$(DESTDIR)$(PREFIX)/bin/xdi-backend"

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
	cp "$(BACKEND_BIN)" LICENSE README.md docs/LICENSES.md docs/licenses-dependencies.md \
		backend/third-party/swagger-ui-5.32.6-licenses.txt release/xdi-backend-$(VERSION)-linux-x86_64/
	tar -C release -czf release/xdi-backend-$(VERSION)-linux-x86_64.tar.gz xdi-backend-$(VERSION)-linux-x86_64
	rm -rf release/xdi-backend-$(VERSION)-linux-x86_64
	git archive --format=tar.gz --prefix=xdi-platform-$(VERSION)/ -o release/xdi-platform-$(VERSION).tar.gz HEAD
	scripts/release-notes.sh $(VERSION) > release/notes.md || { test "$(DRY_RUN)" = 1 && echo "Dry run: CHANGELOG.md has no section for $(VERSION)." > release/notes.md; }

clean: ## Xóa sản phẩm build
	cd backend && cargo clean
