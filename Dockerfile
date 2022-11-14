FROM rust:slim AS builder

RUN cargo new --bin dns-hole

WORKDIR /dns-hole

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release \
 && rm src/*.rs target/release/deps/dns_hole*

COPY ./src ./src
RUN cargo build --release

ENV USER=dns-hole
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

FROM gcr.io/distroless/cc

ENV LISTEN_ADDR=0.0.0.0:3000
EXPOSE 3000

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /dns-hole

COPY --from=builder /dns-hole/target/release/dns-hole ./dns-hole

USER dns-hole:dns-hole

CMD ["/dns-hole/dns-hole"]
