# syntax=docker/dockerfile:1.7

# ---- build stage -----------------------------------------------------------
FROM rust:1.97-slim-bookworm AS builder

# musl target keeps the runtime image small; cross-compile from bookworm.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      pkg-config libssl-dev ca-certificates build-essential musl-tools \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /work

# Cache dependencies by copying only manifests first, then a stub main so
# `cargo build --release` populates the crate cache.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
 && cargo build --release \
 && rm -rf src target/release/deps/stuffy_council*

# Now bring in the real source and templates + migrations (needed at build time
# for the Askama macro and sqlx migrate! call).
COPY src ./src
COPY templates ./templates
COPY migrations ./migrations

RUN cargo build --release

# ---- runtime stage ---------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /app --shell /usr/sbin/nologin app

WORKDIR /app

# Copy binary + the cast files + migrations + the committed allowlist,
# all read from disk at runtime (relative to WORKDIR).
COPY --from=builder /work/target/release/stuffy-council /usr/local/bin/stuffy-council
COPY --chown=app:app cast /app/cast
COPY --chown=app:app migrations /app/migrations
COPY --chown=app:app authorized-users.toml /app/authorized-users.toml

# Data directory for SQLite (mount a persistent volume here in production).
RUN mkdir -p /data && chown app:app /data
VOLUME ["/data"]

USER app
EXPOSE 8080

ENV BIND_ADDR=0.0.0.0:8080 \
    APP_ENV=production \
    DATABASE_URL=sqlite:///data/stuffy.sqlite?mode=rwc

# Tini reaps zombies + forwards signals so graceful shutdown works.
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/stuffy-council"]
