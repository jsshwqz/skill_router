# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /build

# 安装 musl 工具链（静态链接二进制）
RUN apt-get update && apt-get install -y musl-tools pkg-config && \
    rustup target add x86_64-unknown-linux-musl && \
    rm -rf /var/lib/apt/lists/*

# 先拷贝 Cargo 文件利用 Docker 缓存依赖层
COPY Cargo.toml Cargo.lock ./
COPY aion-types/Cargo.toml aion-types/Cargo.toml
COPY aion-memory/Cargo.toml aion-memory/Cargo.toml
COPY aion-intel/Cargo.toml aion-intel/Cargo.toml
COPY aion-router/Cargo.toml aion-router/Cargo.toml
COPY aion-cli/Cargo.toml aion-cli/Cargo.toml
COPY aion-server/Cargo.toml aion-server/Cargo.toml

# 创建 dummy src 触发依赖编译缓存
RUN mkdir -p aion-types/src aion-memory/src aion-intel/src aion-router/src aion-cli/src aion-server/src && \
    echo "pub fn placeholder() {}" > aion-types/src/lib.rs && \
    echo "pub fn placeholder() {}" > aion-memory/src/lib.rs && \
    echo "pub fn placeholder() {}" > aion-intel/src/lib.rs && \
    echo "pub fn placeholder() {}" > aion-router/src/lib.rs && \
    echo "fn main() {}" > aion-cli/src/main.rs && \
    echo "fn main() {}" > aion-server/src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl 2>/dev/null || true

# 拷贝真实源码
COPY . .

# 触发增量编译（真实源码覆盖 dummy）
RUN touch aion-types/src/lib.rs aion-memory/src/lib.rs aion-intel/src/lib.rs \
    aion-router/src/lib.rs aion-cli/src/main.rs aion-server/src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/aion-cli /usr/local/bin/aion-cli
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/aion-server /usr/local/bin/aion-server

# 默认工作目录
WORKDIR /workspace

# aion-server 默认端口
EXPOSE 3000

# 默认启动 HTTP 服务
ENTRYPOINT ["aion-server"]
