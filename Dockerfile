FROM alpine:3.20 AS certs

RUN apk add --no-cache ca-certificates

FROM scratch

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY target/x86_64-unknown-linux-musl/release/oxidepdf-web /var/lib/oxidepdf/oxidepdf-web

WORKDIR /var/lib/oxidepdf
VOLUME ["/var/lib/oxidepdf/upload"]

EXPOSE 19898

ENTRYPOINT ["/var/lib/oxidepdf/oxidepdf-web"]
# 0.0.0.0 is required for the container's published port to be reachable.
# Enable auth by passing OXIDEPDF_AUTH_USER/OXIDEPDF_AUTH_PASS. Without auth,
# also pass OXIDEPDF_ALLOW_UNAUTH_NETWORK=true only behind a trusted boundary.
# Tune per-file upload limits with OXIDEPDF_MAX_UPLOAD, e.g. 256M.
CMD ["--addr", "0.0.0.0", "--port", "19898"]
