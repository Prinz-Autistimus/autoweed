FROM node:24-slim AS frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --no-fund --no-audit
COPY frontend/ ./
RUN npm run build

FROM rust:1-slim-trixie AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p autoweed-server

FROM debian:trixie-slim
COPY --from=build /src/target/release/autoweed-server /usr/local/bin/autoweed-server
COPY --from=frontend /src/frontend/dist /app/static
ENV STATIC_DIR=/app/static
EXPOSE 3000
CMD ["autoweed-server"]
