# Start with a rust alpine image
FROM rust:1.85-bookworm as builder

WORKDIR /opt

COPY Cargo.toml Cargo.lock ./

# Remove benches and tests from Cargo.toml to not affect build on their change.
RUN sed -i '/"benches"/,/"tests"/d' Cargo.toml

COPY crates crates/
COPY apps apps/

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/opt/target \
    cargo build --release -p lrc20d && \
    cargo build --release -p ogaki && \
    mkdir out && \
    cp target/release/lrc20d out/ && \
    cp target/release/ogaki out/ && \
    strip out/lrc20d && \
    strip out/ogaki

# use a plain alpine image, the alpine version needs to match the builder
FROM debian:bookworm-slim

# Copy our build
COPY --from=builder /opt/out/lrc20d /bin/lrc20d
COPY --from=builder /opt/out/ogaki /bin/ogaki

# Optional envvar for GITHUB token to access private assets.
ENV GITHUB_TOKEN=""

ENTRYPOINT /bin/ogaki run-with-auto-update --config /config.toml --token "$GITHUB_TOKEN"