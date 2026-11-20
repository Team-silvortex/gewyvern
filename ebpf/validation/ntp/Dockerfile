FROM alpine@sha256:fd791d74b68913cbb027c6546007b3f0d3bc45125f797758156952bc2d6daf40
RUN apk add --no-cache chrony=4.8-r2
COPY local.conf unsynced.conf /
ENTRYPOINT ["/usr/sbin/chronyd", "-d", "-x", "-u", "root", "-4"]
CMD ["-f", "/local.conf"]
