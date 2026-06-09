FROM rust:alpine3.20 AS chef
RUN apk add --no-cache musl-dev perl make
RUN cargo install cargo-chef --version 0.1.62
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN strip --strip-all target/x86_64-unknown-linux-musl/release/pem_electrolyzer

FROM alpine:3.20 AS runtime
WORKDIR /app
RUN apk add --no-cache ca-certificates tzdata
ENV TZ=Asia/Shanghai
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/pem_electrolyzer /app/pem_electrolyzer
COPY config.toml /app/config.toml
RUN addgroup -S app && adduser -S app -G app
USER app
EXPOSE 8080 34567/udp 9000
ENTRYPOINT ["/app/pem_electrolyzer"]
CMD ["--config", "/app/config.toml"]
