FROM rust:1.92-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --locked -p greenbyte-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home greenbyte
COPY --from=builder /build/target/release/greenbyte-server /usr/local/bin/greenbyte-server
USER 10001:10001
EXPOSE 3030
ENTRYPOINT ["/usr/local/bin/greenbyte-server"]
