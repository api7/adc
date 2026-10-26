FROM rust:1-trixie AS builder

WORKDIR /build

COPY rust ./rust

RUN cd rust && cargo build --release --locked -p adc-cli

FROM gcr.io/distroless/cc-debian13:nonroot

COPY --from=builder /build/rust/target/release/adc /adc

ENTRYPOINT [ "/adc" ]
