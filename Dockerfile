FROM alpine:3.20 AS certs

RUN apk add --no-cache ca-certificates

FROM scratch

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY target/x86_64-unknown-linux-musl/release/oxidepdf /oxidepdf
COPY target/x86_64-unknown-linux-musl/release/completions/oxidepdf.bash /usr/share/bash-completion/completions/oxidepdf.bash

ENTRYPOINT ["/oxidepdf"]
CMD ["--help"]
