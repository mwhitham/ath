# ath-host, the optional program that serves many athletes' records over HTTPS.
# docs/hosting.md explains the configuration and how to run it.
#
# Build from the repository root:
#
#   docker build -f host/Dockerfile -t ath-host .
#
# Run it with a configuration file in /config and the data folder as a volume:
#
#   docker run -v ath-host-data:/data -v /etc/ath-host:/config:ro -p 127.0.0.1:8080:8080 ath-host

# Keep this equal to the channel in rust-toolchain.toml. The install step below reads
# that file, so the build uses the pinned toolchain in any case.
ARG RUST_VERSION=1.100.0

FROM rust:${RUST_VERSION}-slim-trixie AS build
WORKDIR /src
COPY rust-toolchain.toml ./
RUN rustup toolchain install
COPY . .
RUN cargo build --release --locked -p athleticstandard-host --bin ath-host

FROM debian:trixie-slim
# The libSQL client reads the system's certificate authorities for TLS.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
# The host runs as its own user, without administrator rights. Only that user can
# read the data folder.
RUN useradd --system --user-group --uid 10001 --home-dir /data --no-create-home \
        --shell /usr/sbin/nologin ath-host \
    && mkdir -p /data /config \
    && chown ath-host:ath-host /data \
    && chmod 700 /data
COPY --from=build /src/target/release/ath-host /usr/local/bin/ath-host
USER ath-host
VOLUME ["/data"]
EXPOSE 8080
# The host finishes open requests and stops on SIGINT.
STOPSIGNAL SIGINT
ENTRYPOINT ["ath-host"]
CMD ["serve", "--config", "/config/ath-host.json"]
