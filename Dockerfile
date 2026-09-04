# syntax=docker/dockerfile:1

FROM rust:bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang cmake pkg-config libclang-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
ENV CXXFLAGS="-include cstdint"
RUN cargo build --release -p ivory

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 65532 ivory \
    && useradd --uid 65532 --gid 65532 --home-dir /data --shell /usr/sbin/nologin ivory \
    && mkdir -p /data \
    && chown -R 65532:65532 /data
COPY --from=build /src/target/release/ivory /usr/local/bin/ivory
COPY deploy/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod 755 /usr/local/bin/ivory /usr/local/bin/docker-entrypoint.sh
VOLUME ["/data"]
EXPOSE 8545 9000
USER 65532:65532
ENTRYPOINT ["docker-entrypoint.sh"]
