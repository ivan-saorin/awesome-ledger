# awesome-ledger workspace image (DEC-17/DEC-25): thin layer over vm-base
# adding the Rust toolchain for sessions and job runs. No COPY â€” SIGILED
# builds this at master and mounts the repo as /workspace at runtime.
FROM ghcr.io/ivan-saorin/vm-base:0.1.0

USER root
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*

USER 1000:1000
ENV RUSTUP_HOME=/home/dev/.rustup \
    CARGO_HOME=/home/dev/.cargo \
    PATH=/home/dev/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain 1.97.1
