# syntax=docker/dockerfile:1

# ---- ビルド ----
# Rust の版は rust-toolchain.toml と揃える（ずれても rustup が rust-toolchain.toml の版を入れる）
FROM rust:1.98.1-slim-trixie AS builder
WORKDIR /src

# 画面のアセットをバンドルする topcoat-cli。版は Cargo.lock の topcoat に合わせる
COPY Cargo.lock rust-toolchain.toml ./
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    version=$(awk '$0 == "name = \"topcoat\"" { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock) \
    && test -n "$version" \
    && cargo install topcoat-cli --version "^$version" --locked

COPY Cargo.toml ./
COPY crates crates

# topcoat asset bundle は --locked なしで cargo build を呼ぶので、先に --locked で作っておく。
# 書体はバンドルのときに Fontsource からダウンロードする（実行時にインターネットは要らない）
ARG TARGETPLATFORM
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=galley-target-$TARGETPLATFORM,sharing=locked \
    cargo build --release --locked -p galley-server \
    && mkdir -p /out/app /out/data \
    && topcoat asset bundle -p galley-server --release --out /out/app/assets \
    && cp target/release/galley /out/app/

# ---- 実行 ----
# glibc・CA 証明書（S3 への接続）・tzdata（TZ）入りで、シェルの無いイメージ。nonroot は uid 65532
FROM gcr.io/distroless/cc-debian13:nonroot

LABEL org.opencontainers.image.title="Galley" \
      org.opencontainers.image.description="AI が生成した HTML 資料を保存・閲覧・バージョン管理するセルフホスト型の Web アプリ" \
      org.opencontainers.image.source="https://github.com/yuya-take/galley-docs" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0"

# 画面のアセット（assets/）は実行ファイルの隣から読み込む
COPY --from=builder /out/app /app
COPY --from=builder --chown=65532:65532 /out/data /data

ENV GALLEY_DATA_DIR=/data \
    GALLEY_APP_ADDR=0.0.0.0:8080 \
    GALLEY_VIEWER_ADDR=0.0.0.0:8081
# 管理用のサブコマンドを docker exec galley galley purge ... で呼べるようにする（シェルが無いので PATH で探させる）
ENV PATH=/app:${PATH}

# SQLite（galley.db）と資料の実体（blobs/）
VOLUME ["/data"]
# 8080: 画面と MCP（/mcp）、8081: 資料の配信
EXPOSE 8080 8081
USER 65532:65532
ENTRYPOINT ["/app/galley"]
