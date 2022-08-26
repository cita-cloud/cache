FROM rust:slim-bullseye AS buildstage
WORKDIR /build
ENV PROTOC_NO_VENDOR 1
RUN /bin/sh -c set -eux;\
    rustup component add rustfmt;\
    apt-get update;\
    apt-get install -y --no-install-recommends wget;\
    apt-get install -y protobuf-compiler;\
    rm -rf /var/lib/apt/lists/*;\
    GRPC_HEALTH_PROBE_VERSION=v0.4.10;\
    wget -qO /bin/grpc_health_probe https://github.com/grpc-ecosystem/grpc-health-probe/releases/download/${GRPC_HEALTH_PROBE_VERSION}/grpc_health_probe-linux-amd64;\
    chmod +x /bin/grpc_health_probe;
COPY . /build/
RUN cargo build --release
FROM debian:bullseye-slim
COPY --from=buildstage /build/target/release/cache /usr/bin/
COPY --from=buildstage /bin/grpc_health_probe /usr/bin/
CMD ["cache"]
