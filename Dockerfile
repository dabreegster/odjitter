FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /app/target/release/odjitter /usr/local/bin/odjitter
ENTRYPOINT ["/usr/local/bin/odjitter"]
