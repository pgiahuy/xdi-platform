# Dockerfile - một image chạy cả hệ thống: backend Rust phục vụ API dưới /api và giao diện ở các đường dẫn còn lại.
# Build từ gốc repo vì cần cả backend/, frontend/ và data/domains/: docker build -f backend/Dockerfile .
# Rust build nhiều giai đoạn với cargo-chef để cache dependency riêng; sửa code không biên dịch lại crate.

# Giao diện: chép package*.json trước để lớp npm ci chỉ chạy lại khi đổi phụ thuộc.
FROM node:24-bookworm-slim AS web
WORKDIR /web
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# Extension postgres của DuckDB cho SOURCE_SNAPSHOT_MODE=direct (#246): tải lúc build, kiểm SHA-256 theo phiên bản ghi
# trong script, để backend nạp từ file và không bao giờ tải gì lúc chạy. TARGETARCH do BuildKit đặt (amd64, arm64).
# Stage riêng chỉ chép script, nên sửa code backend không tải lại extension.
FROM debian:bookworm-slim AS duckdb-extension
ARG TARGETARCH
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY scripts/fetch-duckdb-extension.sh /fetch-duckdb-extension.sh
RUN /fetch-duckdb-extension.sh /postgres_scanner.duckdb_extension "linux_${TARGETARCH:-amd64}"

FROM rust:1.96-bookworm AS chef
# Số tiến trình biên dịch song song, mỗi tiến trình cần khoảng 1 GiB RAM (docs/INSTALL.md mục 1). ARG có mặt như biến
# môi trường trong các lệnh RUN sau nó, và cargo đọc biến này. "default" là giá trị cargo hiểu là mọi nhân; chuỗi rỗng
# thì cargo báo lỗi, nên docker-compose.yml truyền "default" khi .env để trống. Khai báo trong stage chef, không đặt
# trước FROM đầu tiên: ARG toàn cục không vào stage nào nếu stage đó không khai báo lại, còn ARG của chef thì planner và
# builder (FROM chef) tự kế thừa. Đặt trước cargo install để cả bước cài cargo-chef cũng theo giới hạn.
ARG CARGO_BUILD_JOBS=default
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY backend/ ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
# CARGO_BUILD_JOBS kế thừa từ chef, nên cargo chef cook (biên dịch DuckDB) và cargo build theo giới hạn. Phần C++ của
# DuckDB biên dịch bằng crate cc, xin lượt chạy từ jobserver của cargo nên cũng không vượt số tiến trình này.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY backend/ ./
# Gói khái niệm ngành để backend/build.rs nhúng vào binary; .dockerignore mở riêng data/domains/, dataset lớn trong
# data/ vẫn nằm ngoài context. build.rs tìm ở ../data/domains tính từ crate, tức /data/domains ở stage này.
COPY data/domains/ /data/domains/
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/xdi-backend /usr/local/bin/xdi-backend
COPY --from=web /web/dist /usr/share/xdi/web
# Đường dẫn mặc định của SOURCE_POSTGRES_EXTENSION (backend/src/config.rs). Giấy phép của extension đi cùng.
COPY --from=duckdb-extension /postgres_scanner.duckdb_extension /usr/local/lib/xdi/postgres_scanner.duckdb_extension
COPY backend/third-party/duckdb-postgres-extension-licenses.txt /usr/local/lib/xdi/
ENV STATIC_DIR=/usr/share/xdi/web
# Chạy bằng user không có quyền root.
RUN useradd --system --uid 10001 --no-create-home xdi && mkdir -p /data && chown xdi /data
USER xdi
# Cổng mặc định; đổi bằng BACKEND_PORT.
EXPOSE 8000
CMD ["xdi-backend"]
