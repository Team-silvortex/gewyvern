FROM alpine@sha256:fd791d74b68913cbb027c6546007b3f0d3bc45125f797758156952bc2d6daf40
RUN apk add --no-cache net-snmp=5.9.5.2-r0
COPY snmpd.conf /snmpd.conf
ENTRYPOINT ["/usr/sbin/snmpd", "-f", "-Lo", "-C", "-c", "/snmpd.conf"]
