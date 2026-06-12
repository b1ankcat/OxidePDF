FROM alpine:3.20 AS certs

RUN apk add --no-cache ca-certificates

FROM scratch

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY target/x86_64-unknown-linux-musl/release/oxidepdf /oxidepdf

ENTRYPOINT ["/oxidepdf"]
CMD ["--help"]
