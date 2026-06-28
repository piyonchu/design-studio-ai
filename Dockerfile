# Multi-stage build for the Rust/Axum backend. Migrations are embedded at
# compile time (sqlx::migrate!), so the runtime image carries only the binary.
FROM rust:1-slim AS build
WORKDIR /app
# Build deps first for layer caching (a dummy main so deps compile without src).
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release --locked 2>/dev/null || true
# Now the real sources.
COPY backend/ ./
RUN cargo build --release --locked --bin design-studio-backend

FROM debian:bookworm-slim
# ca-certificates: outbound TLS to Postgres (Neon), OpenRouter, S3.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/design-studio-backend /usr/local/bin/app
# Cloud Run / most PaaS inject $PORT; the app honors it (default 8080).
ENV PORT=8080
EXPOSE 8080
CMD ["app"]
