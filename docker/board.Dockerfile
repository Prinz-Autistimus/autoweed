# Wird in GitHub Actions nativ auf einem arm64-Runner gebaut.
FROM rust:1-slim-trixie AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p autoweed-board

FROM debian:trixie-slim
COPY --from=build /src/target/release/autoweed-board /usr/local/bin/autoweed-board
EXPOSE 8080
CMD ["autoweed-board"]
