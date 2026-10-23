# One container: the Rust server also serves the built game page, so there is a single thing to host.
FROM node:22-slim AS web
WORKDIR /web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
# The public address, so link previews (WhatsApp, Facebook, LinkedIn) point at the right place.
ARG NEXT_PUBLIC_SITE_URL
ENV NEXT_PUBLIC_SITE_URL=$NEXT_PUBLIC_SITE_URL
RUN npm run build

FROM rust:1-slim AS server
WORKDIR /app
COPY server/ ./
RUN cargo build --release

FROM debian:stable-slim
RUN useradd -m app && mkdir /data && chown app /data
WORKDIR /srv
COPY --from=server /app/target/release/kolom-server /srv/kolom-server
COPY --from=web /web/out /srv/web
USER app
ENV STATIC_DIR=/srv/web DB_PATH=/data/kolom.db PORT=8080
EXPOSE 8080
CMD ["/srv/kolom-server"]
