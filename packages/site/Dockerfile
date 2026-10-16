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
COPY packages/site-compiler-service/ ./packages/site-compiler-service/
COPY packages/site/package.json packages/site/Cargo.toml packages/site/Cargo.lock packages/site/build.rs ./packages/site/
COPY packages/site/.cargo ./packages/site/.cargo
COPY packages/site/src/lib.rs ./packages/site/src/lib.rs
COPY packages/site/src/lib/playground/compilation-schema.ts ./packages/site/src/lib/playground/compilation-schema.ts
RUN pnpm install --frozen-lockfile
RUN cargo fetch --locked --manifest-path packages/rustier-native/Cargo.toml \
    && cargo fetch --locked --manifest-path packages/site/Cargo.toml
ENV CARGO_NET_OFFLINE=true
RUN pnpm --filter rustier-native build:release:local \
    && pnpm --filter rustier build:ts \
    && pnpm --filter site-compiler-service build \
    && pnpm --filter site-compiler-service check
RUN mkdir -p packages/site/.rustier/tools/wasm_bindgen/bin \
    && ln -s /opt/cargo/bin/wasm-bindgen packages/site/.rustier/tools/wasm_bindgen/bin/wasm-bindgen

FROM toolchain AS site-build
COPY packages/site/tsconfig.json packages/site/vite.config.ts ./packages/site/
COPY packages/site/dev ./packages/site/dev
COPY packages/site/scripts/cache-examples.ts ./packages/site/scripts/cache-examples.ts
COPY packages/site/src ./packages/site/src
COPY packages/site/static ./packages/site/static
RUN --mount=type=cache,target=/app/packages/site/target,sharing=locked \
    --mount=type=cache,target=/app/packages/site/.rustier,sharing=locked \
    mkdir -p packages/site/.rustier/tools/wasm_bindgen/bin \
      packages/site/.rustier/example-build/.rustier/tools/wasm_bindgen/bin \
    && ln -sf /opt/cargo/bin/wasm-bindgen packages/site/.rustier/tools/wasm_bindgen/bin/wasm-bindgen \
    && ln -sf /opt/cargo/bin/wasm-bindgen packages/site/.rustier/example-build/.rustier/tools/wasm_bindgen/bin/wasm-bindgen \
    && pnpm --filter site build \
    && pnpm --filter site deploy --prod --legacy /site

FROM toolchain AS compiler-build
RUN pnpm --filter site-compiler-service deploy --prod --legacy /compiler
RUN PLAYGROUND_ROOT=/work/project node packages/site-compiler-service/dist/seed.mjs \
    && mkdir -p /opt/playground \
    && cp -a /work/project/. /opt/playground/ \
    && chown -R 1000:1000 /opt/playground

FROM ${NODE_IMAGE} AS site
WORKDIR /app
ENV NODE_ENV=production HOST=0.0.0.0 PORT=3000
COPY --from=site-build /app/packages/site/build ./build
COPY --from=site-build /app/packages/site/package.json ./package.json
COPY --from=site-build /site/node_modules ./node_modules
EXPOSE 3000
USER node
CMD ["node", "build"]

FROM ${NODE_IMAGE} AS worker-runtime
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=compiler-build /opt/rustup /opt/rustup
COPY --from=compiler-build /opt/cargo/bin /opt/cargo/bin
COPY --from=compiler-build /opt/cargo/registry /opt/cargo/registry
COPY --from=compiler-build /opt/playground /opt/playground
COPY --from=compiler-build /compiler /app/packages/site-compiler-service
COPY --from=toolchain /app/packages/rustier-rt/Cargo.toml /app/packages/rustier-rt/Cargo.toml
COPY --from=toolchain /app/packages/rustier-rt/src /app/packages/rustier-rt/src
RUN mkdir -p /work /tmp /proc /dev /app/packages/rustier-native

FROM ${NODE_IMAGE} AS compiler
RUN apt-get update && apt-get install -y --no-install-recommends bubblewrap \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=worker-runtime / /sandbox
COPY --from=compiler-build /compiler ./
EXPOSE 3001
USER node
CMD ["node", "dist/server.mjs"]
