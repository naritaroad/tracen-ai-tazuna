# Release image: the Rust API binary plus the built web site (docs/engineering/architecture.md).
# Build with: docker compose --profile app up -d --build

FROM node:24-trixie-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.99-slim-trixie AS api
WORKDIR /src/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
# Cache dependencies separately from our own code.
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release --locked && rm -rf src
COPY backend/ ./
RUN touch src/main.rs && cargo build --release --locked

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 tracen
WORKDIR /app
COPY --from=api /src/backend/target/release/tracen-api /app/tracen-api
COPY --from=web /src/web/dist /app/web
COPY data/seed /app/data/seed
COPY data/eval /app/data/eval
# Character manifests only; the model and motion files are not redistributable.
COPY assets/characters/9001_hayakawa_tazuna/character.json /app/assets/characters/9001_hayakawa_tazuna/character.json
COPY assets/characters/9001_hayakawa_tazuna/animation/animations.json /app/assets/characters/9001_hayakawa_tazuna/animation/animations.json
COPY assets/characters/9001_hayakawa_tazuna/behavior.json /app/assets/characters/9001_hayakawa_tazuna/behavior.json
# Official academy crest for the site mark and favicon (NOTICE.md).
COPY assets/brand/tracen-crest.svg /app/assets/brand/tracen-crest.svg
USER tracen
EXPOSE 8080
ENTRYPOINT ["/app/tracen-api"]
