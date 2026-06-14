# Reference: Protocol Alias Index

Use this page when you want the current built-in alias map without scanning every protocol package manifest by hand.

This index is intended to stay synchronized with the registry-backed protocol surface in the current tree.

## Format

- `Default entry` is the canonical entry chosen when a family is selected without an explicit entry.
- `Protocol aliases` are accepted family-level spellings that resolve before entry selection.
- `Entry aliases` are accepted per-entry spellings that resolve to a canonical entry name.

## `amqp`

Default entry: `session`  
Protocol aliases: `amqp-consume`, `amqp-publish`, `amqp-session`, `amqp-start`, `amqp_consume`, `amqp_publish`, `amqp_session`, `amqp_start`  
Entry aliases:
- `session` (default): `amqp-session`, `amqp_session`, `connect`
- `consume`: `deliver`, `receive`
- `publish`: `amqp-publish`, `amqp_publish`, `send`
- `start`: `amqp-start`, `amqp_start`, `login`, `negotiate`

## `coap`

Default entry: `get`  
Protocol aliases: `coap-delete`, `coap-post`, `coap-put`, `coap_delete`, `coap_post`, `coap_put`  
Entry aliases:
- `get` (default): none
- `delete`: `destroy`, `remove`
- `post`: `create`, `write`
- `put`: `replace`, `update`

## `dhcp`

Default entry: `client`  
Protocol aliases: `dhcp-discover`, `dhcp-request`, `dhcp_discover`, `dhcp_request`  
Entry aliases:
- `client` (default): none
- `discover`: `lease-discover`, `offer-probe`
- `request`: `lease-request`, `renew`

## `dns`

Default entry: `udp`  
Protocol aliases: none  
Entry aliases:
- `udp` (default): none
- `tcp`: `dns-tcp`, `dns_tcp`

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
- `request` (default): `client`, `http-client`, `http-request`, `http_client`, `http_request`
- `auth-required`: none
- `auth-tunnel`: none
- `connect`: none
- `denied`: none
- `response`: `server`

## `http3`

Default entry: `request`  
Protocol aliases: `h3-request`, `h3-server`, `h3_request`, `h3_server`, `http3-server-response`  
Entry aliases:
- `request` (default): none
- `server`: none

## `https`

Default entry: `connect`  
Protocol aliases: none  
Entry aliases:
- `connect` (default): none

## `hy2`

Default entry: `auth`  
Protocol aliases: `hy2-auth`, `hy2-relay`, `hy2-stream`, `hy2-tcp`, `hy2-udp`, `hysteria2`, `hysteria2-auth`, `hysteria2-tcp`, `hysteria2-udp`  
Entry aliases:
- `auth` (default): none
- `tcp`: none
- `udp`: none

## `imap`

Default entry: `auth`  
Protocol aliases: none  
Entry aliases:
- `auth` (default): `login`
- `auth-denied`: `login-denied`
- `select`: `mailbox`

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
- `bind-denied`: `auth-denied`, `login-denied`
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

## `memcached`

Default entry: `get`  
Protocol aliases: `memcached-get`, `memcached-set`, `memcached_get`, `memcached_set`  
Entry aliases:
- `get` (default): `memcached-get`, `memcached_get`, `read`
- `set`: `memcached-set`, `memcached_set`, `write`

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
Protocol aliases: `mysql-connect`, `mysql-error`, `mysql-query`, `mysql-session`, `mysql_connect`, `mysql_error`, `mysql_query`, `mysql_session`  
Entry aliases:
- `session` (default): `mysql-session`, `mysql_session`
- `connect`: `mysql-connect`, `mysql_connect`
- `error`: `mysql-error`, `mysql_error`
- `query`: `mysql-query`, `mysql_query`

## `ntp`

Default entry: `client`  
Protocol aliases: `ntp-query`, `ntp-sync`, `ntp_query`, `ntp_sync`  
Entry aliases:
- `client` (default): none
- `query`: `check`, `probe`
- `sync`: `clock-sync`, `time-sync`

## `pop3`

Default entry: `auth`  
Protocol aliases: none  
Entry aliases:
- `auth` (default): `login`
- `auth-denied`: `login-denied`
- `list`: `mailbox`

## `postgres`

Default entry: `query`  
Protocol aliases: `postgres-auth`, `postgres-connect`, `postgres-error`, `postgres-query`, `postgres-session`, `postgres_auth`, `postgres_connect`, `postgres_error`, `postgres_query`, `postgres_session`  
Entry aliases:
- `query` (default): `postgres-query`, `postgres_query`
- `auth`: `postgres-auth`, `postgres_auth`
- `connect`: `postgres-connect`, `postgres_connect`
- `error`: `postgres-error`, `postgres_error`
- `session`: `auth-query`, `query-session`

## `quic`

Default entry: `initial`  
Protocol aliases: none  
Entry aliases:
- `initial` (default): none
- `bidi`: none
- `crypto`: none
- `stream`: none

## `radius`

Default entry: `access`  
Protocol aliases: none  
Entry aliases:
- `access` (default): `auth`, `login`

## `redis`

Default entry: `ping`  
Protocol aliases: `redis-get`, `redis-ping`, `redis-session`, `redis-set`, `redis_get`, `redis_ping`, `redis_session`, `redis_set`  
Entry aliases:
- `ping` (default): `health`, `redis-ping`, `redis_ping`
- `blmove`: `blocking-left-right-move`, `blocking-right-left-move`, `list-blocking-directional-move`, `list-blocking-move`
- `blmpop`: `blocking-list-pop-many`, `list-blocking-multi-pop`
- `blpop`: `left-blocking-pop`, `list-blocking-pop-left`
- `brpop`: `list-blocking-pop-right`, `right-blocking-pop`
- `brpoplpush`: `list-blocking-move-right-to-left`, `right-blocking-pop-left-push`
- `bzmpop`: `score-blocking-pop-many`, `sorted-blocking-multi-pop`
- `bzpopmax`: `score-blocking-pop-highest`, `sorted-blocking-pop-max`
- `bzpopmin`: `score-blocking-pop-lowest`, `sorted-blocking-pop-min`
- `decr`: `count-down`, `decrement`
- `del`: `delete`, `remove`
- `exists`: `key-check`, `present`
- `expire`: `expiry`, `set-ttl`
- `get`: `kv-read`, `read`
- `hget`: `field-read`, `hash-read`
- `hmget`: `fields-read`, `hash-multi-read`
- `hmset`: `fields-write`, `hash-multi-write`
- `hset`: `field-write`, `hash-write`
- `incr`: `count-up`, `increment`
- `lmove`: `left-right-move`, `list-directional-move`, `list-move`, `right-left-move`
- `lmpop`: `list-multi-pop`, `list-pop-many`
- `lpop`: `left-pop`, `list-pop-left`
- `lpush`: `left-push`, `list-prepend`
- `mget`: `bulk-read`, `multi-read`
- `mset`: `bulk-write`, `multi-write`
- `pttl`: `ms-ttl`, `precise-ttl`
- `publish`: `channel-write`, `pubsub-send`
- `rpop`: `list-pop-right`, `right-pop`
- `rpoplpush`: `list-move-right-to-left`, `right-pop-left-push`
- `rpush`: `list-append`, `right-push`
- `sadd`: `member-add`, `set-add`
- `session`: `connect`, `roundtrip`
- `set`: `kv-write`, `write`
- `smembers`: `members-read`, `set-read`
- `subscribe`: `channel-read`, `pubsub-listen`
- `ttl`: `key-ttl`, `time-to-live`
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
- `options` (default): `probe`
- `describe`: `metadata`
- `play`: `start`
- `setup`: `stream`

## `sip`

Default entry: `register`  
Protocol aliases: none  
Entry aliases:
- `register` (default): `login`
- `bye`: `hangup`, `terminate`
- `invite`: `call`, `session`

## `smtp`

Default entry: `session`  
Protocol aliases: none  
Entry aliases:
- `session` (default): none
- `auth`: `login`
- `data`: `message`
- `data-denied`: `message-denied`
- `mail`: `sender`
- `rcpt`: `recipient`
- `rcpt-denied`: `recipient-denied`

## `snmp`

Default entry: `get`  
Protocol aliases: `snmp-get-next`, `snmp-set`, `snmp_get_next`, `snmp_set`  
Entry aliases:
- `get` (default): `query`, `read`
- `get-next`: `next`, `walk`
- `set`: `update`, `write`

## `socks5`

Default entry: `session`  
Protocol aliases: `socks`, `socks5-session`, `socks5_session`  
Entry aliases:
- `session` (default): `connect`, `proxy`
- `auth`: `login`, `userpass`
- `auth-connect-denied`: `login-connect-denied`, `userpass-connect-denied`
- `auth-denied`: `login-denied`, `userpass-denied`
- `denied`: `connect-denied`

## `ssdp`

Default entry: `discovery`  
Protocol aliases: none  
Entry aliases:
- `discovery` (default): none

## `ssh`

Default entry: `session`  
Protocol aliases: none  
Entry aliases:
- `session` (default): `connect`, `handshake`
- `auth`: `login`, `password`
- `auth-denied`: `login-denied`, `password-denied`
- `channel`: `shell`

## `stun`

Default entry: `binding`  
Protocol aliases: `stun-allocate`, `stun-refresh`, `stun_allocate`, `stun_refresh`  
Entry aliases:
- `binding` (default): none
- `allocate`: `relay`, `turn-allocate`
- `refresh`: `keepalive`, `turn-refresh`

## `tls`

Default entry: `client`  
Protocol aliases: none  
Entry aliases:
- `client` (default): none

## `wireguard`

Default entry: `handshake`  
Protocol aliases: none  
Entry aliases:
- `handshake` (default): none
