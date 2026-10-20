ARG NODE_IMAGE=node:24.21.0-bookworm-slim@sha256:d6aa754f16b3197301076f047b5def2f02ea1dbbc2ca920407d46d7ec7f87b20
FROM ${NODE_IMAGE} AS toolchain
ARG TARGETARCH
ARG RUST_TOOLCHAIN=nightly-2026-10-14
ARG WASM_BINDGEN_VERSION=0.2.129
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo RUSTUP_TOOLCHAIN=${RUST_TOOLCHAIN}
ENV PATH="/opt/cargo/bin:${PATH}"
RUN case "$TARGETARCH" in \
      arm64) rustup_target=aarch64-unknown-linux-gnu; checksum=e3853c5a252fca15252d07cb23a1bdd9377a8c6f3efa01531109281ae47f841c ;; \
      amd64) rustup_target=x86_64-unknown-linux-gnu; checksum=20a06e644b0d9bd2fbdbfd52d42540bdde820ea7df86e92e533c073da0cdd43c ;; \
      *) exit 1 ;; \
    esac \
    && curl --proto '=https' --tlsv1.2 -fsSL \
      "https://static.rust-lang.org/rustup/archive/1.28.2/$rustup_target/rustup-init" -o /tmp/rustup-init \
    && echo "$checksum  /tmp/rustup-init" | sha256sum -c - \
    && chmod +x /tmp/rustup-init \
    && /tmp/rustup-init -y --default-toolchain "$RUST_TOOLCHAIN" --profile minimal \
      --component rust-src --target wasm32-unknown-unknown \
    && rm /tmp/rustup-init
RUN npm install -g pnpm@12.10.1
RUN cargo install wasm-bindgen-cli --version "=$WASM_BINDGEN_VERSION" --locked
WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml rust-toolchain.toml LICENSE README.md ./
COPY packages/rustier/ ./packages/rustier/
COPY packages/rustier-native/ ./packages/rustier-native/
COPY packages/rustier-rt/ ./packages/rustier-rt/
COPY packages/site-compiler-service/package.json ./packages/site-compiler-service/
RUN pnpm install --frozen-lockfile --ignore-scripts
RUN cargo fetch --locked --manifest-path packages/rustier-native/Cargo.toml
ENV CARGO_NET_OFFLINE=true
RUN pnpm --filter rustier-native build:release:local \
    && pnpm --filter rustier build:ts
COPY packages/site-compiler-service/dist ./packages/site-compiler-service/dist
RUN pnpm --filter site-compiler-service deploy --prod --legacy /compiler
RUN CARGO_NET_OFFLINE=false PLAYGROUND_ROOT=/work/project node packages/site-compiler-service/dist/index.mjs seed \
    && mkdir -p /opt/playground \
    && cp -a /work/project/. /opt/playground/

FROM ${NODE_IMAGE} AS worker-runtime
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=toolchain /opt/rustup /opt/rustup
COPY --from=toolchain /opt/cargo/bin /opt/cargo/bin
COPY --from=toolchain /opt/cargo/registry /opt/cargo/registry
COPY --from=toolchain --chown=node:node /opt/playground /opt/playground
COPY --from=toolchain /compiler /app/packages/site-compiler-service
COPY --from=toolchain /app/packages/rustier-rt/Cargo.toml /app/packages/rustier-rt/Cargo.toml
COPY --from=toolchain /app/packages/rustier-rt/src /app/packages/rustier-rt/src
RUN mkdir -p /work /tmp /proc /dev /app/packages/rustier-native

FROM ${NODE_IMAGE} AS compiler
RUN apt-get update && apt-get install -y --no-install-recommends bubblewrap \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=worker-runtime / /sandbox
COPY --from=toolchain /compiler ./
EXPOSE 3001
USER node
CMD ["node", "dist/index.mjs"]
