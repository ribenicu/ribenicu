# 一本日记 · 后端镜像（多阶段构建）
# 构建：docker build -f deploy/Dockerfile -t riben-server .
# 运行：见 deploy/docker-compose.yml

FROM rust:1.96-bookworm AS builder
WORKDIR /build

# 先拷贝清单，充分利用构建缓存；cargo registry 挂载缓存，重建免重复下载
COPY server/Cargo.toml server/Cargo.lock* ./
COPY server/src ./src
COPY server/migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release && \
    cp target/release/riben-server /riben-server

# ---------- 运行时 ----------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
# migration 已内嵌进二进制（sqlx::migrate!），无需额外拷贝
COPY --from=builder /riben-server /app/riben-server

ENV HOST=0.0.0.0 \
    PORT=9015

EXPOSE 9015
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s \
    CMD curl -fsS http://127.0.0.1:9015/api/health || exit 1

CMD ["/app/riben-server"]
