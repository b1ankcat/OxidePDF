FROM alpine:3.20 AS certs

RUN apk add --no-cache ca-certificates

FROM alpine:3.20 AS fonts

RUN apk add --no-cache fontconfig font-noto-cjk ttf-dejavu

FROM scratch

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=fonts /etc/fonts /etc/fonts
COPY --from=fonts /usr/share/fonts /usr/share/fonts
COPY target/x86_64-unknown-linux-musl/release/oxidepdf-web /var/lib/oxidepdf/oxidepdf-web

WORKDIR /var/lib/oxidepdf
VOLUME ["/var/lib/oxidepdf/upload"]

EXPOSE 19898

ENV OXIDEPDF_AUTH_USER=admin

ENTRYPOINT ["/var/lib/oxidepdf/oxidepdf-web"]
# Defaults listen on all container interfaces and require callers to provide
# OXIDEPDF_AUTH_PASS at runtime. The server rejects the old admin/admin default.
# Tune per-file upload limits with OXIDEPDF_MAX_UPLOAD, e.g. 256M.
CMD ["--addr", "0.0.0.0", "--port", "19898"]
