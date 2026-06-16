FROM alpine:3.20 AS certs

RUN apk add --no-cache ca-certificates

FROM scratch

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY target/x86_64-unknown-linux-musl/release/oxidepdf-web /oxidepdf-web

EXPOSE 19898

ENTRYPOINT ["/oxidepdf-web"]
CMD ["--addr", "0.0.0.0", "--port", "19898"]
