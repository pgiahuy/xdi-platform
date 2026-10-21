# Dockerfile - một image chạy cả hệ thống: backend Rust phục vụ API dưới /api và giao diện ở các đường dẫn còn lại.
# Build từ gốc repo vì cần cả backend/ và frontend/: docker build -f backend/Dockerfile .
# Rust build nhiều giai đoạn với cargo-chef để cache dependency riêng; sửa code không biên dịch lại crate.

# Giao diện: chép package*.json trước để lớp npm ci chỉ chạy lại khi đổi phụ thuộc.
FROM node:24-bookworm-slim AS web
WORKDIR /web
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1.96-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY backend/ ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY backend/ ./
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/xdi-backend /usr/local/bin/xdi-backend
COPY --from=web /web/dist /usr/share/xdi/web
ENV STATIC_DIR=/usr/share/xdi/web
# Chạy bằng user không có quyền root.
RUN useradd --system --uid 10001 --no-create-home xdi && mkdir -p /data && chown xdi /data
USER xdi
# Cổng mặc định; đổi bằng BACKEND_PORT.
EXPOSE 8000
CMD ["xdi-backend"]
