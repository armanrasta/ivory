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
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/ivory /usr/local/bin/ivory
COPY deploy/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh
VOLUME ["/data"]
EXPOSE 8545 9000
ENTRYPOINT ["docker-entrypoint.sh"]
