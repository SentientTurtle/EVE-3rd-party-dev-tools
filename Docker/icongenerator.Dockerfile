FROM rustlang/rust:nightly as builder
WORKDIR /repo

RUN git clone --depth 1 https://github.com/SentientTurtle/EVE-3rd-party-dev-tools.git . && cd eveicongenerator && cargo build --release

FROM ubuntu:latest

RUN apt update && apt install -y ca-certificates

WORKDIR /icongen
COPY --from=builder /repo/eveicongenerator/target/release/eveicongenerator eveicongenerator
COPY --from=builder /repo/Docker/imei.sh imei.sh

RUN imei.sh

VOLUME ["/icongen/icons", "/icongen/cache"]
ENTRYPOINT ["./eveicongenerator"]