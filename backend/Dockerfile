# Dockerfile - build nhiều giai đoạn với cargo-chef để cache dependency riêng; sửa code không biên dịch lại crate.
FROM rust:1.96-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/xdi-backend /usr/local/bin/xdi-backend
# Chạy bằng user không có quyền root.
RUN useradd --system --uid 10001 --no-create-home xdi && mkdir -p /data && chown xdi /data
USER xdi
EXPOSE 8000
CMD ["xdi-backend"]
