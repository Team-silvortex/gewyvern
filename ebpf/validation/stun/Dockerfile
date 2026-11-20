FROM alpine@sha256:fd791d74b68913cbb027c6546007b3f0d3bc45125f797758156952bc2d6daf40
RUN apk add --no-cache coturn=4.10.0-r0
COPY turnserver.conf /turnserver.conf
ENTRYPOINT ["/usr/bin/turnserver", "-c", "/turnserver.conf"]
