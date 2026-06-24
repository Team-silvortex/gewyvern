# Reference: Protocol Alias Index

Use this page when you want the current built-in alias map without scanning every protocol package manifest by hand.

This index is intended to stay synchronized with the registry-backed protocol surface in the current tree.

## Format

- `Default entry` is the canonical entry chosen when a family is selected without an explicit entry.
- `Protocol aliases` are accepted family-level spellings that resolve before entry selection.
- `Entry aliases` are accepted per-entry spellings that resolve to a canonical entry name.

## `amqp`

Default entry: `session`  
Protocol aliases: `amqp-auth-denied`, `amqp-consume`, `amqp-publish`, `amqp-session`, `amqp-start`, `amqp_auth_denied`, `amqp_consume`, `amqp_publish`, `amqp_session`, `amqp_start`  
Entry aliases:
- `session` (default): `amqp-session`, `amqp_session`, `connect`
- `auth-denied`: `amqp-auth-denied`, `amqp_auth_denied`, `login-denied`, `negotiate-denied`
- `consume`: `amqp-consume`, `amqp_consume`, `deliver`, `receive`
- `publish`: `amqp-publish`, `amqp_publish`, `send`
- `start`: `amqp-start`, `amqp_start`, `login`, `negotiate`

## `coap`

Default entry: `get`  
Protocol aliases: `coap-delete`, `coap-post`, `coap-put`, `coap_delete`, `coap_post`, `coap_put`  
Entry aliases:
- `get` (default): none
- `delete`: `coap-delete`, `coap_delete`, `destroy`, `remove`
- `post`: `coap-post`, `coap_post`, `create`, `write`
- `put`: `coap-put`, `coap_put`, `replace`, `update`

## `dhcp`

Default entry: `client`  
Protocol aliases: `dhcp-discover`, `dhcp-nak`, `dhcp-request`, `dhcp_discover`, `dhcp_nak`, `dhcp_request`  
Entry aliases:
- `client` (default): none
- `discover`: `dhcp-discover`, `dhcp_discover`, `lease-discover`, `offer-probe`
- `nak`: `dhcp-nak`, `dhcp_nak`, `lease-denied`, `offer-denied`
- `request`: `dhcp-request`, `dhcp_request`, `lease-request`, `renew`

## `dns`

Default entry: `udp`  
Protocol aliases: none  
Entry aliases:
- `udp` (default): none
- `tcp`: `dns-over-tls`, `dns-tcp`, `dns_over_tls`, `dns_tcp`, `dot`

## `ftp`

Default entry: `session`  
Protocol aliases: none  
Entry aliases:
- `session` (default): `control`, `login`
- `active-list`: `active-directory`
- `active-retr`: `active-download`
- `active-stor`: `active-upload`
- `denied`: `login-denied`
- `list`: `directory`
- `retr`: `download`
- `stor`: `upload`

## `gtpu`

Default entry: `echo`  
Protocol aliases: `gtp-u`, `gtp_u`  
Entry aliases:
- `echo` (default): `gtp-u`, `gtp_u`

## `http`

Default entry: `request`  
Protocol aliases: `http-connect`, `http-connect-auth-required`, `http-connect-auth-tunnel`, `http-connect-denied`, `http-request`, `http-server`, `http_connect`, `http_connect_auth_required`, `http_connect_auth_tunnel`, `http_connect_denied`, `http_request`, `http_server`  
Entry aliases:
- `request` (default): `client`, `dns-over-https`, `dns_over_https`, `doh`, `http-client`, `http-request`, `http_client`, `http_request`
- `auth-required`: `http-connect-auth-required`, `http_connect_auth_required`
- `auth-tunnel`: `http-connect-auth-tunnel`, `http_connect_auth_tunnel`
- `connect`: `http-connect`, `http_connect`
- `denied`: `http-connect-denied`, `http_connect_denied`
- `response`: `http-server`, `http_server`, `server`

## `http3`

Default entry: `request`  
Protocol aliases: `h3-request`, `h3-server`, `h3_request`, `h3_server`, `http3-server-response`  
Entry aliases:
- `request` (default): `h3-request`, `h3_request`
- `close`: `connection-close`, `connection_close`, `h3-close`, `h3_close`, `http3-close`, `http3_close`, `terminate`
- `server`: `h3-server`, `h3_server`, `http3-server-response`, `http3_server_response`
- `server-close`: `h3-server-close`, `h3_server_close`, `http3-server-close`, `http3_server_close`, `response-close`, `response_close`, `server-close`, `server_close`

## `https`

Default entry: `connect`  
Protocol aliases: none  
Entry aliases:
- `connect` (default): none

## `hy2`

Default entry: `auth`  
Protocol aliases: `hy2-auth`, `hy2-relay`, `hy2-stream`, `hy2-tcp`, `hy2-udp`, `hysteria2`, `hysteria2-auth`, `hysteria2-tcp`, `hysteria2-udp`  
Entry aliases:
- `auth` (default): `hy2-auth`, `hysteria2`, `hysteria2-auth`
- `close`: `hy2-close`, `hy2_close`, `hysteria2-close`, `hysteria2_close`, `session-close`, `session_close`, `terminate`
- `tcp`: `hy2-stream`, `hy2-tcp`, `hysteria2-tcp`
- `tcp-close`: `hy2-tcp-close`, `hy2_tcp_close`, `hysteria2-tcp-close`, `hysteria2_tcp_close`, `stream-close`, `stream_close`, `tcp-close`, `tcp_close`
- `udp`: `hy2-relay`, `hy2-udp`, `hysteria2-udp`
- `udp-close`: `datagram-close`, `datagram_close`, `hy2-udp-close`, `hy2_udp_close`, `hysteria2-udp-close`, `hysteria2_udp_close`, `udp-close`, `udp_close`

## `imap`

Default entry: `auth`  
Protocol aliases: none  
Entry aliases:
- `auth` (default): `imap-auth`, `imap_auth`, `login`
- `auth-denied`: `imap-auth-denied`, `imap_auth_denied`, `login-denied`
- `select`: `imap-select`, `imap_select`, `mailbox`

## `kerberos`

Default entry: `as`  
Protocol aliases: none  
Entry aliases:
- `as` (default): `initial-auth`, `login`
- `as-error`: `initial-auth-error`, `login-denied`
- `tgs`: `service-ticket`, `ticket`

## `ldap`

Default entry: `sync`  
Protocol aliases: `ldap-bind`, `ldap-bind-denied`, `ldap-constraint`, `ldap-denied`, `ldap-modify`, `ldap-search`, `ldap-session`, `ldap-sync`, `ldap-write`, `ldap_bind`, `ldap_bind_denied`, `ldap_constraint`, `ldap_denied`, `ldap_modify`, `ldap_search`, `ldap_session`, `ldap_sync`, `ldap_write`  
Entry aliases:
- `sync` (default): `ldap-sync`, `ldap_sync`, `replication`
- `bind`: `auth`, `ldap-bind`, `ldap_bind`, `login`
- `bind-denied`: `auth-denied`, `ldap-bind-denied`, `ldap_bind_denied`, `login-denied`
- `constraint`: `ldap-constraint`, `ldap_constraint`
- `denied`: `ldap-denied`, `ldap_denied`
- `modify`: `ldap-modify`, `ldap_modify`
- `search`: `directory`, `ldap-search`, `ldap_search`, `query`
- `session`: `directory-session`, `ldap-session`, `ldap_session`
- `write`: `ldap-write`, `ldap_write`

## `mdns`

Default entry: `query`  
Protocol aliases: none  
Entry aliases:
- `query` (default): none
- `probe`: `claim`, `conflict-check`, `mdns-probe`, `mdns_probe`
- `response`: `announcement`, `answer`, `mdns-response`, `mdns_response`

## `memcached`

Default entry: `get`  
Protocol aliases: `memcached-get`, `memcached-set`, `memcached_get`, `memcached_set`  
Entry aliases:
- `get` (default): `memcached-get`, `memcached-read`, `memcached_get`, `memcached_read`, `read`
- `set`: `memcached-set`, `memcached-write`, `memcached_set`, `memcached_write`, `write`

## `mqtt`

Default entry: `connect`  
Protocol aliases: none  
Entry aliases:
- `connect` (default): `login`, `session`
- `disconnect`: `close`, `teardown`
- `pubcomp`: `complete`, `qos2-complete`
- `publish`: `message`, `send`
- `pubrec`: `qos2-receipt`, `stage-2`
- `pubrel`: `qos2-release`, `resume`
- `subscribe`: `listen`, `read`

## `mysql`

Default entry: `session`  
Protocol aliases: `mysql-auth`, `mysql-auth-denied`, `mysql-connect`, `mysql-error`, `mysql-query`, `mysql-session`, `mysql_auth`, `mysql_auth_denied`, `mysql_connect`, `mysql_error`, `mysql_query`, `mysql_session`  
Entry aliases:
- `session` (default): `mysql-session`, `mysql_session`
- `auth`: `mysql-auth`, `mysql_auth`
- `auth-denied`: `handshake-denied`, `login-denied`, `mysql-auth-denied`, `mysql_auth_denied`
- `connect`: `mysql-connect`, `mysql_connect`
- `error`: `mysql-error`, `mysql_error`
- `query`: `mysql-query`, `mysql_query`

## `ntp`

Default entry: `client`  
Protocol aliases: `ntp-query`, `ntp-sync`, `ntp_query`, `ntp_sync`  
Entry aliases:
- `client` (default): none
- `query`: `check`, `ntp-query`, `ntp_query`, `probe`
- `sync`: `clock-sync`, `ntp-sync`, `ntp_sync`, `time-sync`

## `pop3`

Default entry: `auth`  
Protocol aliases: none  
Entry aliases:
- `auth` (default): `login`, `pop3-auth`, `pop3_auth`
- `auth-denied`: `login-denied`, `pop3-auth-denied`, `pop3_auth_denied`
- `list`: `mailbox`, `pop3-list`, `pop3_list`

## `postgres`

Default entry: `query`  
Protocol aliases: `postgres-auth`, `postgres-auth-denied`, `postgres-connect`, `postgres-error`, `postgres-query`, `postgres-session`, `postgres_auth`, `postgres_auth_denied`, `postgres_connect`, `postgres_error`, `postgres_query`, `postgres_session`  
Entry aliases:
- `query` (default): `postgres-query`, `postgres_query`
- `auth`: `postgres-auth`, `postgres_auth`
- `auth-denied`: `login-denied`, `password-denied`, `postgres-auth-denied`, `postgres_auth_denied`
- `connect`: `postgres-connect`, `postgres_connect`
- `error`: `postgres-error`, `postgres_error`
- `session`: `auth-query`, `postgres-session`, `postgres_session`, `query-session`

## `quic`

Default entry: `initial`  
Protocol aliases: none  
Entry aliases:
- `initial` (default): none
- `bidi`: none
- `close`: `connection-close`, `connection_close`, `quic-close`, `quic_close`, `terminate`
- `crypto`: none
- `local-close`: `active-close`, `active_close`, `local-close`, `local_close`, `quic-local-close`, `quic_local_close`
- `retry`: `address-validation`, `quic-retry`, `quic_retry`, `token-challenge`
- `stream`: none

## `radius`

Default entry: `access`  
Protocol aliases: `radius-challenge`, `radius-denied`, `radius_challenge`, `radius_denied`  
Entry aliases:
- `access` (default): `auth`, `login`, `radius-access`, `radius_access`
- `challenge`: `mfa`, `otp`, `radius-challenge`, `radius_challenge`
- `denied`: `access-denied`, `login-denied`, `radius-denied`, `radius_denied`, `reject`

## `redis`

Default entry: `ping`  
Protocol aliases: `redis-get`, `redis-ping`, `redis-session`, `redis-set`, `redis_get`, `redis_ping`, `redis_session`, `redis_set`  
Entry aliases:
- `ping` (default): `health`, `redis-ping`, `redis_ping`
- `ask`: `cluster-ask`, `slot-ask`
- `auth-denied`: `login-denied`, `wrongpass`
- `auth-required`: `login-required`, `noauth`
- `blmove`: `blocking-left-right-move`, `blocking-right-left-move`, `list-blocking-directional-move`, `list-blocking-move`
- `blmpop`: `blocking-list-pop-many`, `list-blocking-multi-pop`
- `blpop`: `left-blocking-pop`, `list-blocking-pop-left`
- `brpop`: `list-blocking-pop-right`, `right-blocking-pop`
- `brpoplpush`: `list-blocking-move-right-to-left`, `right-blocking-pop-left-push`
- `busy`: `lua-blocked`, `script-busy`
- `busygroup`: `consumer-group-exists`, `stream-group-exists`
- `bzmpop`: `score-blocking-pop-many`, `sorted-blocking-multi-pop`
- `bzpopmax`: `score-blocking-pop-highest`, `sorted-blocking-pop-max`
- `bzpopmin`: `score-blocking-pop-lowest`, `sorted-blocking-pop-min`
- `clusterdown`: `cluster-unavailable`, `slot-map-down`
- `crossslot`: `cluster-slot-conflict`, `multi-key-slot-conflict`
- `decr`: `count-down`, `decrement`
- `del`: `delete`, `remove`
- `error`: `command-error`, `resp-error`
- `execabort`: `multi-exec-abort`, `transaction-abort`
- `exists`: `key-check`, `present`
- `expire`: `expiry`, `set-ttl`
- `get`: `kv-read`, `read`, `redis-get`, `redis_get`
- `hget`: `field-read`, `hash-read`
- `hmget`: `fields-read`, `hash-multi-read`
- `hmset`: `fields-write`, `hash-multi-write`
- `hset`: `field-write`, `hash-write`
- `incr`: `count-up`, `increment`
- `lmove`: `left-right-move`, `list-directional-move`, `list-move`, `right-left-move`
- `lmpop`: `list-multi-pop`, `list-pop-many`
- `loading`: `loading-window`, `warmup-busy`
- `lpop`: `left-pop`, `list-pop-left`
- `lpush`: `left-push`, `list-prepend`
- `masterdown`: `failover-window`, `primary-unavailable`
- `mget`: `bulk-read`, `multi-read`
- `misconf`: `persistence-misconfig`, `write-guarded`
- `moved`: `cluster-redirect`, `slot-moved`
- `mset`: `bulk-write`, `multi-write`
- `noscript`: `evalsha-miss`, `script-missing`
- `oom`: `memory-limit`, `write-over-capacity`
- `pttl`: `ms-ttl`, `precise-ttl`
- `publish`: `channel-write`, `pubsub-send`
- `readonly`: `readonly-replica`, `replica-write-denied`
- `rpop`: `list-pop-right`, `right-pop`
- `rpoplpush`: `list-move-right-to-left`, `right-pop-left-push`
- `rpush`: `list-append`, `right-push`
- `sadd`: `member-add`, `set-add`
- `session`: `connect`, `redis-session`, `redis_session`, `roundtrip`
- `set`: `kv-write`, `redis-set`, `redis_set`, `write`
- `smembers`: `members-read`, `set-read`
- `subscribe`: `channel-read`, `pubsub-listen`
- `tryagain`: `backoff-retry`, `cluster-retry`
- `ttl`: `key-ttl`, `time-to-live`
- `wrongtype`: `type-conflict`, `wrong-type`
- `xack`: `stream-ack`, `stream-acknowledge`
- `xadd`: `stream-append`, `stream-write`
- `xautoclaim`: `stream-auto-claim`, `stream-idle-reassign`
- `xclaim`: `stream-claim`, `stream-reassign`
- `xdel`: `stream-delete`, `stream-prune-entry`
- `xgroup`: `stream-consumer-group`, `stream-group`, `stream-group-create`, `stream-group-create-consumer`, `stream-group-destroy`, `stream-group-drop-consumer`, `stream-group-help`, `stream-group-list-consumers`, `stream-group-list-groups`, `stream-group-manage`, `stream-group-setid`
- `xinfo`: `stream-info`, `stream-info-consumers`, `stream-info-groups`, `stream-info-stream`, `stream-inspect`
- `xlen`: `stream-count`, `stream-length`
- `xpending`: `stream-delivery-backlog`, `stream-pending`
- `xrange`: `stream-history`, `stream-range`
- `xread`: `stream-consume`, `stream-read`
- `xreadgroup`: `stream-consumer-read`, `stream-group-read`
- `xrevrange`: `stream-history-reverse`, `stream-range-reverse`
- `xtrim`: `stream-prune`, `stream-trim`
- `zadd`: `score-add`, `sorted-add`
- `zcard`: `score-count`, `sorted-count`
- `zcount`: `score-window-count`, `sorted-range-count`
- `zincrby`: `score-bump`, `sorted-score-increment`
- `zmpop`: `score-pop-many`, `sorted-multi-pop`
- `zpopmax`: `score-pop-highest`, `sorted-pop-max`
- `zpopmin`: `score-pop-lowest`, `sorted-pop-min`
- `zrange`: `score-read`, `sorted-read`
- `zrangebyscore`: `score-window-read`, `sorted-range-score`
- `zrank`: `score-rank-member`, `sorted-member-rank`
- `zrem`: `score-remove`, `sorted-remove`
- `zrevrangebyscore`: `score-window-read-reverse`, `sorted-revrange-score`
- `zrevrank`: `score-revrank-member`, `sorted-member-revrank`
- `zscore`: `score-read-member`, `sorted-member-score`

## `rtsp`

Default entry: `options`  
Protocol aliases: `rtsp-describe`, `rtsp-options`, `rtsp-play`, `rtsp-setup`, `rtsp_describe`, `rtsp_options`, `rtsp_play`, `rtsp_setup`  
Entry aliases:
- `options` (default): `probe`, `rtsp-options`, `rtsp_options`
- `describe`: `metadata`, `rtsp-describe`, `rtsp_describe`
- `play`: `rtsp-play`, `rtsp_play`, `start`
- `setup`: `rtsp-setup`, `rtsp_setup`, `stream`

## `sip`

Default entry: `register`  
Protocol aliases: none  
Entry aliases:
- `register` (default): `login`, `sip-register`, `sip_register`
- `bye`: `hangup`, `sip-bye`, `sip_bye`, `terminate`
- `denied`: `4xx`, `5xx`, `6xx`, `failed`, `rejected`, `sip-denied`, `sip_denied`
- `invite`: `call`, `session`, `sip-invite`, `sip_invite`
- `response`: `final`, `provisional`, `reply`, `sip-response`, `sip_response`

## `smtp`

Default entry: `session`  
Protocol aliases: none  
Entry aliases:
- `session` (default): none
- `auth`: `login`
- `auth-denied`: `login-denied`
- `data`: `message`
- `data-denied`: `message-denied`
- `mail`: `sender`
- `rcpt`: `recipient`
- `rcpt-denied`: `recipient-denied`

## `snmp`

Default entry: `get`  
Protocol aliases: `snmp-bulk`, `snmp-engine-sync`, `snmp-get-next`, `snmp-report`, `snmp-set`, `snmp-trap`, `snmp-trap-recv`, `snmp-unauthorized`, `snmp-v3-auth`, `snmp-v3-priv`, `snmp_bulk`, `snmp_engine_sync`, `snmp_get_next`, `snmp_report`, `snmp_set`, `snmp_trap`, `snmp_trap_recv`, `snmp_unauthorized`, `snmp_v3_auth`, `snmp_v3_priv`  
Entry aliases:
- `get` (default): `query`, `read`
- `bulk`: `bulk-walk`, `snmp-bulk`, `snmp_bulk`, `table-read`
- `engine-sync`: `engine-discovery`, `report-sync`, `snmp-engine-sync`, `snmp_engine_sync`
- `get-next`: `next`, `snmp-get-next`, `snmp_get_next`, `walk`
- `inform`: `ack-notify`, `confirm-notify`
- `report`: `engine-report`, `report-pdu`, `snmp-report`, `snmp_report`
- `set`: `snmp-set`, `snmp_set`, `update`, `write`
- `trap`: `alert`, `notify`, `snmp-trap`, `snmp_trap`
- `trap-recv`: `listen-trap`, `snmp-trap-recv`, `snmp_trap_recv`, `trap-listener`
- `unauthorized`: `access-denied`, `auth-failed`, `snmp-unauthorized`, `snmp_unauthorized`
- `v3-auth`: `auth-session`, `auth-user`, `snmp-v3-auth`, `snmp_v3_auth`
- `v3-priv`: `encrypted-session`, `private-session`, `snmp-v3-priv`, `snmp_v3_priv`

## `socks5`

Default entry: `session`  
Protocol aliases: `socks`, `socks5-session`, `socks5_session`  
Entry aliases:
- `session` (default): `connect`, `proxy`, `socks`, `socks5-session`, `socks5_session`
- `auth`: `login`, `userpass`
- `auth-connect-denied`: `login-connect-denied`, `userpass-connect-denied`
- `auth-denied`: `login-denied`, `userpass-denied`
- `denied`: `connect-denied`

## `ssdp`

Default entry: `discovery`  
Protocol aliases: none  
Entry aliases:
- `discovery` (default): none
- `notify`: `advertise`, `alive`, `byebye`, `ssdp-notify`, `ssdp_notify`

## `ssh`

Default entry: `session`  
Protocol aliases: none  
Entry aliases:
- `session` (default): `connect`, `handshake`, `ssh-session`, `ssh_session`
- `auth`: `login`, `password`, `ssh-auth`, `ssh_auth`
- `auth-denied`: `login-denied`, `password-denied`, `ssh-auth-denied`, `ssh_auth_denied`
- `channel`: `shell`, `ssh-channel`, `ssh_channel`

## `stun`

Default entry: `binding`  
Protocol aliases: `stun-allocate`, `stun-binding-error`, `stun-refresh`, `stun_allocate`, `stun_binding_error`, `stun_refresh`  
Entry aliases:
- `binding` (default): none
- `allocate`: `relay`, `stun-allocate`, `stun_allocate`, `turn-allocate`
- `binding-error`: `binding-denied`, `binding-error`, `stun-binding-error`, `stun_binding_error`
- `refresh`: `keepalive`, `stun-refresh`, `stun_refresh`, `turn-refresh`

## `tls`

Default entry: `client`  
Protocol aliases: none  
Entry aliases:
- `client` (default): `initiator`, `tls-client`, `tls_client`
- `server`: `acceptor`, `tls-server`, `tls_server`

## `wireguard`

Default entry: `handshake`  
Protocol aliases: none  
Entry aliases:
- `handshake` (default): none
- `cookie`: `cookie-reply`, `wireguard-cookie`, `wireguard_cookie`
- `transport`: `data`, `session`, `wireguard-data`, `wireguard_data`
