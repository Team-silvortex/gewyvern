# Field Findings

This note records the highest-signal findings inherited by the active `2.5.x` line
while preserving a shorter historical tail from the earlier `0.15.x`
field-validation phase.

It is intentionally short.

It is not a replacement for:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)

Instead, it answers a narrower question for the stable line:

- what has already been observed in real validation work
- what currently looks stable
- what still looks conservative rather than strongly diagnostic

## Role In The Shelf

Treat this page as the short evidence ledger.

Use it when the question is:

- what has already been demonstrated recently?
- which release-line claims are backed by concrete observed runs?
- where did we already discover drift or conservative gaps?

Do not use this page as:

- the validation program definition
- the shortest release gate
- the main statement of what the stable line should mean

For those, use:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)

## `2026-11-13` RTSP Control Requires Real Header Semantics

Three fresh physical-Linux `--rtsp-breadth` campaigns passed against
digest-pinned MediaMTX 1.21.3. Six selected connections retained 39 packet
events, 30 TCP state events and 4,434 server-response bytes with zero reported
loss. All 33 selected commands, 12 package replays, three excluded same-target
neighbors and three post-cleanup HTTP captures passed. A separate POP3 control
passed. See the [source-bound RTSP proof](fixtures/gewyvern_rtsp_linux_x86_64_20261113.json).

The old four RTSP packages missed every terminal response: their header-name
fragments assumed a fixed byte offset, but real CSeq/Content-Base fields change
that layout. Merely broadening to `200` or `Session` would be unsafe: the real
461 SETUP refusal also carries Session, while keepalive and PAUSE return 200.
The new bounded packet-local predicate parses a complete header within 512
bytes, independently of field order/case, then requires Public, SDP content
metadata, Session+Transport, or Session+RTP-Info for the respective candidate.
Duplicate/malformed/truncated headers, wrong direction/port, body-only decoys
and ambiguous successes are rejected. Export/replay preserves the new predicate
and rejects invalid ports, unknown kinds and unsupported extra fields.

An uncaptured native Rust publisher provides private SDP before capture. The
client verifies OPTIONS, missing-path DESCRIBE (404 then EOF), fresh-connection
SDP lookup, UDP SETUP refusal followed by TCP SETUP, PLAY, GET_PARAMETER, PAUSE,
resumed PLAY and TEARDOWN. Client CSeq/session checks do not become product
dialogue-correlation claims. Every emitted receive stage is independently
checked against its actual source packet, and every server byte is reconciled.
The initial fixture's assumption that MediaMTX keeps a 404 connection open was
corrected; that development run and the old-rule failure are excluded from totals.

Existing 15 containers, six networks and 54 volumes were preserved. One pinned
MediaMTX image was pulled only on Linux; no Mac build/image work or global prune
was used. Broader API testing also found and fixed a stale SMTP signal assertion.
Core/protocol regressions passed 1,572 tests, API semantics 23, and evidence/status
closure 348, for 1,943 passes with one existing ignored test. Strict clippy and
formatting passed. The temporary build container was removed; the small local
archive excludes build objects and Docker configuration. These checks do not
count as additional media capability.

This is RTSP/1.0 plaintext control evidence, not RTSP/2.0, authentication/TLS,
UDP media, RTP delivery quality, throughput or general TCP reconstruction.
Default sparse collectors still report unsupported offsets. A signature-shaped
body beginning at a packet boundary needs context this predicate does not have.
The catalog now has real exchanges for 43 packages, not full-catalog validation;
no maturity or product version is promoted.

## `2026-11-13` POP3 Mailbox Retrieval And Recovery

Three final physical-Linux `--pop3-breadth` campaigns passed against fresh
digest-pinned Mailpit 1.33.0 targets. Six selected connections retained 111
packet events, 30 TCP state events and 3,789 complete server-response bytes,
with zero reported loss. Sixty selected commands, nine package replays,
three excluded same-target neighbors and three fresh HTTP captures passed.
A separate SMTP control passed. See the
[source-bound POP3 proof](fixtures/gewyvern_pop3_linux_x86_64_20261113.json).

Real responses exposed three missed package outcomes: signed-in success,
invalid-password denial and counted LIST readiness. Old fixtures had sparse,
misaligned text bytes and did not represent the real server. Package rules,
standalone DSL, API semantics and regression fixtures now agree on conservative
English signatures. Bare `+OK` is deliberately unclassified: USER, NOOP and RSET
share it. Unauthorized STAT and missing/deleted-message errors are not password
denial. LIST counts accept one to three decimal digits, not a hard-coded two.

The larger multiline exchange also exposed a validation-tool limit: bpftool's
raw plus decoded event JSON exceeded the generic 256-KiB stdout cap at only 37
events. The fixed 128-slot breadth map now has a dedicated 2-MiB export budget;
ordinary tool limits, stderr limits, deadlines and loss rejection remain intact.
An intermediate LIST fix incorrectly used flow-wide `all` conditions; the actual
packet cross-check caught STAT being used as LIST provenance. Final alternatives
each contain all byte checks in one packet predicate. A multi-packet regression
preserves the correct source fact; no global IR semantics were changed.

Native SMTP setup seeds two messages before capture. The selected POP3 dialogue
checks authentication refusal, unauthorized STAT, LIST/UIDL/RETR, dot-stuffed
lines and status-looking body text, missing messages, DELE refusal on later
RETR, and RSET restoring both counts and list. Complete raw server streams match
the independently received bytes; the setup is not captured request evidence.

The pinned server has a separate length discrepancy: RETR advertises stored raw
length, then its sendData helper adds another CRLF. Every retrieved message is
two bytes longer after unstuffing. This is explicitly reported with
`size_conformant:false` and exact declared/actual lengths, not hidden by trimming
capture data. LIST/STAT agree with the declared lengths; bodies equal the seeded
content plus that exact extra CRLF. Unexpected sizes or content still fail.
The [upstream RETR code](https://github.com/axllent/mailpit/blob/v1.33.0/internal/pop3/server.go)
and [sendData helper](https://github.com/axllent/mailpit/blob/v1.33.0/internal/pop3/functions.go)
explain the observed target behavior. Passing this fixture does not certify
Mailpit's POP3 conformance.

Failed development runs `dev-1`, `dev-2`, `dev-3` and `run-1` are excluded from
the final `final-1`/`final-2`/`final-3` totals. Existing 15 containers, six networks
and 54 volumes were preserved; owned targets/networks were removed, with no
global pruning or Mac image/build work. This is bounded plaintext mailbox
evidence, not TLS/APOP/SASL, arbitrary stream/dialogue decoding, alternative
server coverage, external delivery, deletion-commit durability or throughput.
An exact signature-shaped body at a packet boundary still needs context that
these packet-local product rules do not supply. Maturity remains unchanged.
Final Linux regression, API and evidence/status suites passed 1,906 tests with
one existing ignored test. Strict clippy and formatting checks passed. The
temporary build container was removed; the local evidence archive is about 7.1 MiB.

## `2026-11-13` SMTP Separates Command Replies From Message Outcomes

Three final physical-Linux `--smtp-breadth` campaigns passed against fresh
digest-pinned Mailpit 1.33.0 targets. Six selected connections retained 69 packet
events, 30 TCP state events and 3,078 complete server-response bytes with zero
reported loss. Three same-target neighbors were excluded; 24 package replays
and three fresh HTTP captures passed. See the
[source-bound SMTP proof](fixtures/gewyvern_smtp_linux_x86_64_20261113.json).
The failed `dev-1` capture is retained separately, not included in final totals.

Real responses exposed two production false positives: an unauthenticated
connection's NOOP `250 2.0.0 Ok` became message-queue success, and recipient
rejection `550 5.1.0` became post-body message rejection. Regression checks also
exposed generic `250` being used as EHLO success and recipient `2.1.5` being
used as sender acceptance. The reverse message/recipient-denial ambiguity is
now guarded too. Package and standalone rules were updated together, and every
emitted receive stage is checked against its actual packet rather than only
checking the package's terminal cookie set.

EHLO candidates now require `250-`, sender acceptance requires `250 2.1.0 `,
and queue candidates require the explicit English `250 2.0.0 Ok: queued as `
prefix. Recipient candidates use address failures `550 5.1.0 `/`550 5.1.1 `;
message-rejection candidates use `550 5.7.1 Message rejected`. These signatures
are intentionally conservative, not general command/reply correlation or proof
of transaction completion. Single-line EHLO, alternative/localized text and
split prefixes may remain unmatched. Real post-body denial was not triggered;
that package's real coverage is negative-only, with synthetic positive regression.

Across final runs the selected clients made 57 commands/body submissions,
including six accepted messages after command-order and recipient errors.
The native client checks multiline EHLO, invalid credentials, unauthenticated
MAIL refusal, NOOP, DATA-before-envelope, recipient rejection, two transactions
separated by RSET, a null sender, dot-stuffed body input and QUIT. Server reply
bytes are independently reconciled with captured bytes. Message bodies remain
uncaptured client input; acceptance does not prove mailbox delivery or persistence.

Targets used internal networks, disposable plaintext credentials, bounded tmpfs,
no published ports, no host data mounts and no external forwarding. Existing
container/network/54-volume inventories stayed unchanged after owned cleanup.
A separate MQTT/NATS control passed. No images were pulled or built on Mac.
TLS, external delivery, IMAP/POP3, other SMTP implementations, arbitrary stream
decoding, throughput and independent-host/kernel qualification remain open gates.

## `2026-11-13` MQTT/NATS Extend Real Broker Coverage

Three final physical-Linux `--messaging-breadth` campaigns passed against fresh
digest-pinned Mosquitto and NATS targets. Twelve selected connections retained
66 packet events, 60 TCP state events and 4,770 complete server-response bytes,
with zero reported capture loss. Six same-target neighbors were excluded;
27 package replays and three new HTTP captures passed. See the
[source-bound messaging proof](fixtures/gewyvern_messaging_linux_x86_64_20261113.json).
Final runs are `run-4` through `run-6`; four failed development attempts and three
successful preliminary captures are excluded from these totals.

Real Mosquitto exposed two production rule bugs. The MQTT connection success
predicate accepted only a new session, missing the valid resumed-session
`20 02 01 00` response. PUBACK was incorrectly pinned to packet identifier zero,
missing the real acknowledgement for ID 7. Package and standalone rules now
accept resumed sessions and variable observed PUBACK ID bytes. Fresh/resumed
success, refusals, reserved flags, truncated headers and different IDs have
regression coverage. The fixed native client separately checks exact nonzero
IDs; the production prefix candidate is not an ID-correlation engine.

MQTT checks empty-ID refusal, an uncaptured persistent-session seed, reconnect,
QoS 1/2 acknowledgements, binary delivery through the resumed subscription,
SUBACK and PINGRESP. NATS checks bad credentials, successful subscribe/delivery,
empty/binary/control-looking bodies, denied publication and later delivery on
the same connection. Across final runs, selected clients issued 27 MQTT and
48 NATS commands; no request payloads were captured. In-memory session resume
and acknowledgements do not prove broker crash durability or downstream storage.

The native NATS oracle also needed to accept asynchronous protocol-1 INFO updates.
These remain bounded and present in both raw bytes and decoded transcripts,
without replacing a command reply. Negative message bodies now precede the real
permission error so stage deduplication cannot hide an earlier false positive;
a final message verifies recovery. Earlier successful captures were rerun after
this stronger ordering, rather than silently reused as final-source proof.

Owned targets and networks were removed, and the existing container/network/
54-volume inventories were unchanged. A separate MySQL/Memcached control passed.
Images were prepared only on Linux, using a separate anonymous Docker config
after an expired existing login token was rejected; original credentials were
not changed. This is bounded plaintext broker coverage, not MQTT 5, TLS,
JetStream/cluster, arbitrary stream decoding, throughput or cross-host acceptance.

## `2026-11-13` MySQL/Memcached Fix Real Response Misclassification

Three final physical-Linux `--mysql-memcached` campaigns passed on kernel
`7.0.0-42-generic`, using digest-pinned MySQL 8 and Memcached 1.6 targets.
Twelve selected connections retained 81 packet events, 60 TCP state events and
3,105 complete response bytes with zero reported capture loss. Six same-target
neighbors were excluded; 21 package replays and three fresh HTTP captures passed.
See the [source-bound MySQL/Memcached proof](fixtures/gewyvern_mysql_memcached_linux_x86_64_20261113.json).
Four development attempts are excluded, including two Memcached startup/config
failures. The startup failures remain failures, not protocol acceptance evidence.

The real targets exposed four production rule gaps:

- MySQL query error `1064/42000` also became an authentication-denied candidate.
  Denial now requires `1045/#28000`; the query-error path requires a complete
  protocol-4.1 ERR prefix at command response sequence 1.
- Memcached GET miss status `0x0001` also became a value candidate. Success now
  requires status zero, four flags-extra bytes and an observed 24-byte header.
- Memcached oversized SET rejection `0x0003` became a stored candidate. Success
  now requires status zero and the observed header end.
- Actual APPEND-to-missing-key `NOT_STORED` (`0x0005`) was missed by a SET-only
  rule. SET/APPEND/PREPEND alternatives now share the same status-checked phase;
  APPEND is real-target evidence, while PREPEND also has synthetic regression.

Package and standalone DSL rules were updated together. The validator checks
each matched packet's actual status, rather than letting a later success on the
same socket hide an earlier false positive. MySQL exercises wrong credentials,
five queries including a syntax error, NULL/empty rows, recovery and PING.
Memcached exercises missing keys, rejected writes, unknown commands, successful
reads after errors and empty/binary values. Selected clients issued 15 MySQL
query messages and 39 Memcached commands across the final campaigns.

These fixtures use isolated disposable credentials, internal networks and no
published ports or host data mounts. Images were prepared only on Linux; the
runner itself never pulls images. Full framed client responses are independent
oracles, not invented kernel facts. Seven-byte reads do not force TCP segments.
All owned targets/networks were removed and the existing container/network/
54-volume inventories stayed unchanged. A separate PostgreSQL/Redis control
campaign also passed; its counts are not included in the totals above.
This does not prove TLS, arbitrary TCP reassembly, process attribution, request
payload capture, MySQL caching-SHA2/auth switching/prepared statements,
Memcached text/meta/SASL/UDP, throughput or cross-host/kernel breadth.

## `2026-11-13` Database Boundaries Preserve Transaction State And Byte Values

Three additional physical-Linux `--data-breadth` runs passed against fresh real
PostgreSQL/Redis targets. Twelve selected connections retained 120 packet events,
60 TCP state events and 4,515 complete server-response bytes with zero reported
loss. Six unselected neighbors stayed excluded; eighteen package replays and
three fresh HTTP captures passed. See the [transaction/value boundary proof](fixtures/gewyvern_data_boundaries_linux_x86_64_20261113.json).
The previous schema-1 campaign remains bound to its original source revision and
is retained unchanged; these new totals are separate, not cumulative.
Final-source campaigns are `run-4` through `run-6`; three successful pre-final
development runs are excluded after a test-only static-check spelling fix.

PostgreSQL now exercises ten simple-query messages per selected authenticated
connection, including `BEGIN` and `ROLLBACK`. The failed transaction remains in
ReadyForQuery `E`; the next query returns `25P02` rather than a row. Only rollback
returns to `I`, after which row `45` arrives. SQL NULL and empty text remain
distinct. Redis adds empty bulk values, an eight-byte value containing NUL,
non-UTF-8 data, CRLF and `-ERR`, and three pipelined GET replies. Across the three
runs, selected clients issued 30 PostgreSQL simple-query messages and 63 Redis
commands, including nine pipelined commands. No request payload was captured.

The bounded validation parser previously supported only non-null text rows,
idle readiness and UTF-8 bulk values. Schema 2 now records transaction states
and command tags, emits JSON null for SQL NULL and preserves non-UTF-8 Redis
bytes as hex without lossy replacement. This extends the test oracle, not the
production runtime into a general database driver. Seven-byte client reads
exercise partial I/O; they do not force TCP packet segmentation. Synthetic
fragmented-write/malformed-frame unit controls remain separate from kernel facts.

A new synthetic negative regression also exposed a production rule gap: the
SQLSTATE end-byte predicate checked presence but accepted a nonzero byte. Both
PostgreSQL error package and standalone rules now require the zero terminator.
The before-fix failure is archived; valid real server responses still match.
Canonical English field-layout limitations remain explicit.

All six owned targets and networks were removed; existing container/network/
54-volume inventories were unchanged. No image pulls, host data mounts or public
ports were used. The existing streaming ABI is unchanged. This does not close
TLS/SCRAM, extended queries, general TCP reassembly, RESP3/arrays/cluster,
throughput or independent-host/kernel gates.

## `2026-11-13` PostgreSQL/Redis Expose And Fix An Error Classification Bug

Three physical-Linux `linux-tcp-capture --data-breadth` runs used fresh internal
PostgreSQL 16 and Redis 8 targets on kernel `7.0.0-42-generic`. Twelve selected
connections retained 78 packet events and 60 TCP state events with zero reported
loss. Six same-target unselected neighbors were excluded. Complete server streams
totaled 2,985 bytes and matched native client I/O byte-for-byte from observed
SYN-ACK origins. See the [source-bound database/cache proof](fixtures/gewyvern_data_breadth_linux_x86_64_20261113.json).
Three development runs are excluded from these final totals.

The real PostgreSQL target exposed a production false positive: the old
authentication-denial predicate matched any `E` message, including a successful
login's later division-by-zero query error. Framed responses distinguish SQLSTATE
`28P01` from `22012`. The authentication and query-error package/standalone rules
now require different conservative English severity/SQLSTATE prefixes; crossed
errors, a lone `E` and missing sampled end bytes have regression coverage.
This is not a general decoder: reordered/localized fields, split prefixes and
fatal query failures can remain unmatched.

PostgreSQL returned rows `42` and `43` around the failed query on the same
connection. Redis exercised NOAUTH, WRONGPASS, authenticated SET/GET, missing-key
nil, WRONGTYPE, unknown-command ERR, successful GET after errors, and a normal
bulk value whose text begins `-ERR`. Across the final runs the selected clients
issued nine PostgreSQL queries and 42 Redis commands; request payloads were not
captured. Authentication-message, ReadyForQuery and bulk-response candidates
must not be promoted to login success, query success or key existence.

Six existing packages produced eighteen identical JSON replays. The bounded
parser is shared between independent client and kernel byte sources, not an
independent-parser oracle. Raw packet facts are not resegmented at message
boundaries; missing process/route probes stay explicitly unavailable. The new
finite 1,024-byte prefix uses a per-CPU scratch map, retaining the existing
64-byte streaming prefix and 128-byte event ABI. Streams exceeding 4 KiB,
truncated prefixes, gaps, conflicting overlaps and reported loss fail closed.

All six owned targets and networks were removed. Container/network/54-volume
inventories were unchanged, with no published ports or host data mounts. Redis
was preloaded on Linux during preparation; the runner never pulls images. Data
uses bounded tmpfs and disposable fixture credentials. Fresh 64-byte HTTP
capture passed after every campaign. Builds and images stayed off the Mac.

This closes bounded plaintext database/cache correctness and same-connection
recovery, not throughput, generalized TCP reassembly, TLS/SCRAM, extended queries,
RESP3/cluster/persistence, SQL-injection detection, arbitrary process attribution
or independent-host/kernel acceptance.

## `2026-11-13` UDP/DNS Adds Real Bidirectional Datagram Evidence

Three physical-Linux `linux-udp-capture` runs on kernel `7.0.0-42-generic`
used fresh internal CoreDNS targets and six selected UDP sockets. Across 27
queries, all 27 request and 30 response datagrams were captured with zero
reported loss. Three same-target unselected neighbors were excluded. Each run
covered A-answer success, FORMERR/SERVFAIL/NXDOMAIN/REFUSED, a TC reply and a
bounded dropped-response timeout followed by success on the same socket.
See the [source-bound UDP/DNS proof](fixtures/gewyvern_udp_dns_linux_x86_64_20261113.json).

The real target exposed an incorrect one-response-per-query assumption in the
new fixture: its FORMERR and REFUSED templates each emitted two datagrams.
The client now retains up to four responses in a fixed 50-ms observation window;
every response must match the captured bytes and question. No datagram is
silently dropped or replaced from client data. This behavior follows the target's
[template response path](https://github.com/coredns/coredns/blob/v1.15.0/plugin/template/template.go)
and [error-return convention](https://github.com/coredns/coredns/blob/v1.15.0/plugin/plugin.go).
The recorded elapsed time includes that observation window and is not RTT.
Three development runs are excluded from the final totals.

Three production gaps were fixed: short UDP payloads could match DNS error
codes without an observed header end, only NXDOMAIN opened DNS-error IR, and
alternative RCODEs could be treated as missing sequential stages. The package
and standalone DSL example now declare/check offsets 2, 3 and 11; each RCODE
independently produces IR without missing-alternative findings. Regression
tests cover payload lengths 0-11, missing sampled bytes, QR=0, NOERROR, wrong
port/direction/transport, and unrelated operations/modules. These predicates
remain packet-local candidates, not a general DNS decoder.

The same captured facts produced six DNS package projections with deterministic
JSON replay. Only packet metadata was supplied: no TCP transitions, process
identity or route facts were invented. Two sockets sharing one DNS ID stayed
separate; nine queries per run are not nine runtime flows. TC and timeout were
not promoted to successful resolution or an invented RCODE. All owned resources
were removed, the container/network/54-volume inventories were unchanged, and
fresh HTTP capture passed after each run. The official image was preloaded only
on Linux; the runner never pulls images or changes the host resolver.

This closes a bounded IPv4 DNS correctness slice, not general UDP load, recursive
resolution/cache behavior, IPv6/fragment reassembly, EDNS/DNSSEC/DoH/DoT, TCP
fallback, arbitrary process attribution or independent-host/kernel acceptance.

## `2026-11-13` Real FTP/LDAP Responses Close A Protocol Breadth Gap

Three independent physical-Linux `--auth-breadth` campaigns used fresh private
vsftpd and OpenLDAP containers and unmodified protocol packages. Across twelve
selected connections, FTP 530/230 and LDAP 49/0 responses matched independently
parsed client responses byte-for-byte. Six additional denied connections to the
same targets were excluded by owned socket identity. All 42 packet events and
60 TCP state events were retained without reported loss. See the
[source-bound breadth proof](fixtures/gewyvern_auth_breadth_linux_x86_64_20261113.json).
The recorded final-build campaigns are `run-4` through `run-6`; three earlier
development campaigns are excluded from these totals. Partial response reads
and writes share an absolute exchange deadline instead of restarting a timeout
for each fragment.

The same facts exercised `ftp/denied`, `ftp/session`, `ldap/bind-denied` and
`ldap/bind`: twelve package projections preserved facts, flows, reasons, stages
and IR through JSON replay. Each matching stage referenced a captured packet.
The generic LDAP bind-response phase is not an authentication-success label;
the framed BER result code supplies that distinction. Process and route probes
declared by the packages but not collected here are now explicitly marked
unavailable in the generalized capture adapter. No process identity, route or
request-side payload fact was fabricated.

The old container cleanup could leave anonymous image-declared volumes behind.
Owned-container cleanup now includes anonymous volumes and this campaign checks
their absence. All six target containers, six internal networks and six FTP
volumes were removed. Existing container identities and the 54-volume inventory
were unchanged. No images were pulled or ports published, and fresh HTTP
capture/replay passed after each campaign.

This is evidence of bounded plaintext authentication diagnosis across text and
binary protocols, not arbitrary protocol recognition or exploit detection.
Responses must fit the complete captured ranges; truncated packet prefixes,
gaps, conflicts, malformed frames and unsupported extensions are rejected.
Beyond the later bounded UDP/DNS and PostgreSQL/Redis slices above, IPv6,
encrypted-transport observability, additional database/cache protocols, FTP data
channels and LDAP search/modify/SASL remain separate breadth gates.

## `2026-11-13` Hour-Scale Soak Separates Live Objects From Observer And Cache Retention

Three twenty-minute physical-Linux capture/runtime windows in one process
delivered and analyzed **287,594,379** measured events with zero ring loss. Each
window ended with full teardown and thirty seconds idle; fresh HTTP capture and
reanalysis passed afterward. The two-core/512-MiB fixture, 1-MiB ring, two clients
and independent 256-fact batches were unchanged. No builds or tests ran during
load. See the [source-bound hour proof](fixtures/gewyvern_capture_memory_hour_linux_x86_64_20261113.json).

The earlier diagnostic retained sample trees and compact reports in memory.
That observer bias is now removed: complete samples stream to private JSONL
files through two fixed 8-KiB buffers; cycle reports are dropped before clean
checkpoints. The final result tree is assembled afterward. A validator-only,
opt-in forwarding System allocator counts live requested Rust bytes and blocks
across threads without allocation inside its callbacks. Services retain their
normal allocator, even when the feature is enabled. Audited throughput has
additional counter overhead and is not an ordinary-build performance comparison.

All three clean checkpoints returned to **6,529 live Rust bytes / 80 blocks**,
one thread and four observed descriptors. Load RSS peaks were 26,604 / 43,088 /
43,600 KiB; clean RSS was 24,428 / 24,716 / 27,416 KiB. The final diagnostic-only
trim reclaimed 16,944 KiB, reducing RSS to 10,472 KiB without changing live Rust
counts. No forced trim occurred during load. This establishes reusable free-page
retention rather than equating resident pages with live application objects.

The remaining default-cache nonfree estimates were 130,476 / 156,176 / 171,409
bytes. Two separate no-traffic controls each performed 256 main-thread and 256
worker-thread snapshots, retaining no history until measurement ended. Rust live
counts stayed exactly flat within each series. With normal caching, observer
nonfree estimates grew; worker exit released 26,428 bytes of that estimate.
With `glibc.malloc.tcache_count=0` applied only to the diagnostic process, all
256 main-thread estimates were exactly 7,646 bytes and all 256 worker estimates
exactly 10,526 bytes. Output-argument lengths explain the different absolute
Rust baselines between processes; comparisons use each series' own baseline.

A separate cache-disabled **three-by-five-minute real capture** control then
delivered another 73,906,197 events without loss. Its clean Rust counters were
6,540 bytes / 80 blocks throughout; nonfree estimates warmed from 35,468 to
38,283 bytes and stayed exactly 38,283 in the third cycle. Recovery passed again.
This supports cache fill, allocator bookkeeping and reclaimable pages as the
observed growth mechanisms. It does not provide exact attribution of every C
allocation, indefinite leak freedom, continuous cross-batch session coverage,
or independent-host/kernel breadth. No production cache setting or automatic
trimming policy was introduced. The historical schema-1 proof below is preserved
as originally measured, not relabeled as report-free evidence.

## `2026-11-13` Fifteen-Minute Same-Process Soak Identifies Allocator Retention

Three consecutive 300-second capture/runtime windows, each followed by teardown
and thirty seconds idle, delivered and analyzed 72,427,397 measured kernel events
without ring loss. They ran on one physical Linux 7.0.0-42/glibc 2.43 host with the
existing two-core/512-MiB fixture limits and independent 256-fact batches. No
builds or tests ran concurrently. Delivery was 82,326 / 76,930 / 82,150 events/s;
analysis-completion p99 was 22.608 / 22.684 / 22.693 ms. This is a retention
diagnostic, not a new no-capture throughput comparison. See the
[source-bound memory soak proof](fixtures/gewyvern_capture_memory_soak_linux_x86_64_20261113.json).

Sampled load RSS peaks were 26,840 / 39,664 / 37,332 KiB. After teardown and idle,
RSS was 20,764 / 17,872 / 22,944 KiB; every cycle returned to one thread and four
observed descriptors. All eight allocator arenas remained reusable. The final
snapshot classified 22,558,560 bytes as free arena space versus approximately
194,736 nonfree bytes. A single diagnostic-only trim reduced RSS from 22,944 to
10,576 KiB, releasing 12,368 KiB. Fresh HTTP capture/replay then passed. No trim
occurred during load, and no production allocator setting was changed.

This supports allocator retention as the dominant observed RSS increase, not a
demonstrated batch-object leak. The allocator's nonfree estimate includes metadata,
caches and compact diagnostic reports; its small per-cycle increase is not fully
attributed or certified leak-free. A separate warmed, thread-local Rust allocation
regression covers normal completion, abandoned partial batches and rejected input:
all 131,186,836 requested bytes and 799,705 allocations were released. That test
does not account for C allocations, RSS, other execution paths or indefinite runs.

The new `--capture-soak` entrypoint also keeps missing/duplicate allocator totals
fail-closed. Real-host preflight exposed glibc 2.43's omission of the old fastbin
totals; the parser now handles both schemas explicitly and rejects a per-arena
fastbin whose global total is missing. Private exclusive snapshot files reject
existing files and symlinks. Longer sessions, slow residual growth, paused-consumer
retention and additional physical hosts/kernels remain separate follow-ups.

## `2026-11-13` Load Recovery Passes And Installed Helper Boundaries Are Hardened

The owned-loopback runtime campaign ran on the physical Linux 7.0.0-42 host,
with two clients, a two-core quota, 512 MiB limit, 1-MiB ring and independent
256-fact analysis batches. Both sixty-second normal windows were complete:
4,525,422 and 4,676,915 measured events, with zero ring output failures.
Mean analyzed delivery was 76,665 events/s against the same campaign's 91,478
requests/s no-capture baseline (ratio 0.8381). CPU was approximately the full
two-core quota for the entire fixture, not just the collector. Analysis p99
upper bounds were 23.411/23.200 ms. See the
[source-bound reliability proof](fixtures/gewyvern_ebpf_reliability_linux_x86_64_20261113.json).

Pausing consumption for about two seconds lost 176,969 of 793,060 events;
the remaining 616,091 were delivered and analyzed. Accounting reconciled and
the phase remained explicitly incomplete. Its p99 exceeded the 100-ms histogram
range and stayed null, not zero; the exact maximum was about two seconds.
The following normal window and fresh HTTP capture/replay passed. Separate
real-kernel map loss, event-budget overflow, snapshot-gap, error, abort and
SIGKILL cases all rejected incomplete evidence and recovered with fresh capture.

No new capture correctness failure was observed in this bounded campaign.
Later-window process RSS still increased by 1,288 KiB and peaked at 60,348 KiB
(58.93 MiB); after capture, baseline RSS was 47,488 KiB. No OOM or memory-limit
event occurred. This does not establish long-duration leak freedom, and retained
RSS must not be equated with live collector allocations or cgroup charges.

The preflight did expose an installed-policy failure: the generated nonterminal
wildcard was rejected by the host's default sudo-rs. The template and native
provisioner now use terminal wildcards, with exact command arity/order and
bounded identifiers still enforced by the helper. Both policy parsers accepted
the template; the native provisioner prevalidated the installed policy. The
original UID 1001 stayed authorized; missing device arguments and arbitrary
passwordless commands remained rejected. No unrestricted sudo rule was added.

The fixed helper was also still version 2.0.0 and its source path design relied
on the build checkout. Five allowlisted BPF/loader sources now ship inside the
binary and are exclusively materialized in a private directory. This removes
runtime dependency on mutable or removed workspace C files. The installed helper
was updated to the current Cargo package version 2.5.0 (the source commit is
labelled 2.6.1; this is not a release artifact). Dedicated-account tracepoint,
kprobe and TC tests passed through default sudo on an owned dummy interface.
The old binary/policy backups remain on Linux only. This is one physical host
and one kernel, not a replacement for the wider compatibility matrix.

## `2026-11-11` Mixed Packetization Validates Byte Ranges Beyond Fixed Echoes

The native `linux-tcp-capture --capture-mixed` campaign extends the owned-loopback
checks beyond the fixed 64-byte echo shape. Two physical Linux runs each completed
four fresh-socket epochs: variable logical sizes (1/7/8/63/64/65/511/4096 bytes),
split writes, four-message coalesced exchanges, and a repeated variable case.
Every epoch includes a same-cgroup, same-listener unselected neighbor, plus
client write-half-close before the final response is read and verified.
See the [source-bound mixed-capture proof](fixtures/gewyvern_capture_mixed_linux_x86_64_20261111.json).

For the two selected clients, each epoch has 2,048 logical messages and
1,232,640 response bytes. Both runs observed these different event counts:

| Write Pattern | Logical Messages | Captured Events |
| --- | ---: | ---: |
| Variable-size writes | 2,048 | 2,048 |
| Split writes | 2,048 | 6,144 |
| Four-message coalesced exchanges | 2,048 | 512 |

The acceptance oracle uses per-cookie TCP sequence and payload byte ranges,
not message/event equality. Across both runs, 21,504 actual kernel events were
analyzed in 84 bounded runtime batches, with eight same-adapter JSON reanalyses.
Kernel output failures and malformed events were zero. The selected response
ranges total 9,861,120 bytes; only 645,120 prefix bytes were actually captured
and checked. Full-byte application integrity is independently verified and must
not be presented as full-payload capture. The eight unselected neighbors echoed
another 4,930,560 bytes without entering the capture. All 24 socket identities
were distinct across both campaigns.

Wrong prefixes, foreign identities, framing errors, gaps and overlaps are
rejected rather than silently repaired. TCP sequence wraparound and rejection
atomicity have unit coverage; the live runs did not deliberately force a wrap
or introduce retransmissions. Fresh post-campaign HTTP boundary capture/replay
passed twice. No new correctness failure was observed within this scope.

The sampled process RSS high-water was 26.42 MiB and the largest observed
kernel-to-batch-analysis completion latency was 33.20 ms. These short fixtures
include setup/replay and batch-fill effects: they do not establish sustained
throughput, resolve the previous long-run RSS question, or prove generic TCP
reassembly, vulnerability detection, cross-batch sessions or multi-host support.
Linux regression passed 1,331 functional/contract tests, including explicit
Unix/TCP inputs. Strict Clippy and formatting passed. Owned BPF links, pins and
cgroups were cleaned, temporary containers removed, and the administrative
session closed. Other running services were left unchanged; macOS received
reports only and performed no compilation or container-image pull.

## `2026-11-11` Separate Release CPU From Quota Wait Before Reusing Payload Buffers

The capture adapter now pairs stage wall time with consumer-thread CPU time.
In the two instrumented pre-optimization normal windows, release accumulated
7.79/7.39 s of wall time but 5.02/5.19 s of CPU time, including warmup.
The remainder is a non-CPU residual, not a direct quota-wait measurement.
Fixed-input runs under a quarter-core quota took 0.412-0.587 s of wall time
but 0.110-0.137 s of CPU; cgroup throttling increased by 0.301-0.447 s.
The same input under a two-core quota recorded no new throttling. Clock overhead
is explicit, and short conversion/ingest CPU samples are not extrapolated.
See the [source-bound CPU/allocation proof](fixtures/gewyvern_runtime_allocation_profile_linux_x86_64_20261111.json).

Allocation accounting identified repeated payload-index construction/destruction.
The private capture adapter now reuses at most 256 maps with at most 64 payload
entries each, without changing public facts or export formats. Values are fully
overwritten; length/offset mismatches rebuild the map. Invalid inputs cannot
consume a cached buffer. Recycling happens only after batch checks and sampled
reanalysis; cached values are freed when the sink is dropped.

For three 32,768-record fixed-input runs per version, conversion allocations
fell from 294,912 to 34,816 (88.19%), and batch-release deallocations from
303,488 to 74,112 (75.58%). Median release CPU fell from 30.763 to 1.077 ms;
whole timed-region CPU fell from 106.420 to 75.652 ms (28.91%). These are
test-only System call counts, not memory-residency measurements. Setup and
teardown are outside that timed region. Separate final sink teardown measured
0.367-0.387 ms of CPU and 6,360 deallocations, releasing 1,085,939 requested
bytes, including its catalog and histogram as well as retained payload maps.

The first optimized physical Linux campaign retained the same two clients,
two-core quota, 1-MiB ring and 256-fact batches. Its two normal windows analyzed
5,003,184 and 5,142,345 measured events with zero loss. Average release CPU per
batch fell from 297.55 to 13.36 us. Mean throughput rose from 70,758 to 84,519
events/s, but the no-capture baseline also rose from 92,970 to 103,343 requests/s.
Therefore the raw 19.45% gain is not wholly attributed to this patch: the
capture/baseline ratio changed from 0.7611 to 0.8179. This is a sequential
comparison, not a randomized causal estimate. Analysis p99 upper bounds changed
from 26.633/26.725 ms to 22.713/22.819 ms; quota-driven tails remain visible.

The later normal-window RSS peak rose from 35.52 to 44.25 MiB, despite bounded
pool occupancy. Do not call this a memory optimization or infer long-run leak
freedom from short samples. Per-phase cgroup memory charges and process RSS
have different scopes. Intentional pause still caused 200,396 output failures,
exactly matching sequence gaps; the following normal phase was complete, and
fresh post-pressure HTTP capture/replay passed. No transport, payload-integrity
or OOM/limit failure occurred. Linux functional/contract regression passed 1,328
tests; the profiling fixture passed 15 runs. Strict Clippy and formatting passed.

A final-source confirmation campaign delivered another 5,198,639/5,192,261
normal-window events without loss, averaging 86,573 events/s against its own
104,336 requests/s baseline. Release CPU remained 12.67/12.82 us per batch.
The later RSS peak was again higher, at 44.43 MiB, with 604 KiB growth within
that sixty-second window. This confirms the high-water observation, not its
cause or long-term stability. Pause loss reconciled at 198,560 events and fresh
capture/replay passed again. All three campaigns cleaned their owned BPF links,
pins and cgroups; temporary containers were removed, seven unrelated running
containers were unchanged, and the administrative session was closed. macOS
received reports only, without compiling or pulling container images.

## `2026-11-11` Explicit Catalog Reuse Removes The Runtime Batch Hotspot

Bounded stage timing identified repeated protocol-directory discovery inside
every batch export, including batches whose custom operation has no protocol IR.
`RuntimeExportContext` now lets callers retain an immutable catalog summary and
reuse it through `export_bundle_with_context`/`into_export_bundle_with_context`.
The continuous runtime adapter uses one context per phase, including sampled
reanalysis. Original export methods still rediscover; there is no global cache.
Tests cover output equality, aliases, stable old snapshots and explicit refresh.
See the [source-bound profile and live proof](fixtures/gewyvern_runtime_catalog_profile_linux_x86_64_20261111.json).

Three identical synthetic 32,768-record runs before and after the change gave
median timed-region durations of 3.404 s and 0.103 s. Median analysis time per
256-fact batch fell from 25.564 ms to 86.32 us. Setup is excluded from those
durations: the optimized context also performs one catalog discovery, with a
27.04-ms median. These are fixed-input hotspot measurements, not live packet
throughput, and stage timers measure wall time rather than CPU time.

The subsequent physical Linux five-phase campaign retained the 1-MiB ring,
256-fact batches, two clients and two-core/512-MiB/32-task limits. Its two
sixty-second normal windows delivered and analyzed 4,809,997 and 4,783,236
events, with zero output failures or sequence gaps. Mean throughput was
79,913 events/s versus a same-run no-capture baseline of 102,750 requests/s.
The preceding pre-optimization runtime campaign averaged 8,206 events/s and
lost about 88%; this sequential comparison is not a randomized experiment.

Mean kernel-to-batch-analysis latency was 1.731/1.714 ms; p99 upper bounds were
26.929/26.896 ms and maxima 31.644/31.949 ms. The tail remains visible rather
than being replaced by the mean. Both windows used approximately two cores of
whole-test CPU and showed quota throttling. RSS peaks were 25.53/29.34 MiB;
neither short samples nor a bounded active batch establish long-duration leak
freedom. No transport, byte-integrity or memory-limit/OOM failure occurred.

Pausing the consumer for 2.0002 seconds still caused 197,472 output failures,
exactly matching observed sequence gaps. That phase remains incomplete, while
the following normal window was complete again. Fresh post-pressure HTTP
capture/replay passed. Build settings still optimize the project package only,
with dev-profile dependencies. This is opt-in bounded packet-batch analysis,
not a blanket performance claim for existing callers, durable storage or
continuous cross-batch sessions; no release gate or tensor maturity is promoted.

Linux regression passed 1,322 functional/contract tests, including protocol
registry, protocol IR, failure paths and explicit Unix/TCP input. The separate
profiling fixture passed all six before/after runs; seven existing performance
benchmarks were not run. Strict Clippy and formatting passed. Owned capture
resources and temporary containers were removed; seven unrelated running service
containers were unchanged. The administrative session was closed, and macOS
received 276 KiB of reports without any build or image pull.

## `2026-11-11` Bounded Runtime Batch Analysis Exposes Sustained Overload

This is the historical pre-catalog-reuse measurement; the section above records
the subsequent optimization without replacing these original observations.

`linux-tcp-capture --capture-runtime` now sends every delivered event through
actual `RuntimeSession` ingestion, flow/program/reason analysis and bundle
construction. Independent batches are capped at 256 facts; completed results
are checked and released, rather than accumulating an ever-growing session.
Representative batches undergo JSON roundtrip and fresh-session reanalysis with
the same native capture adapter and DSL rules. This recomputes program results
instead of relying on generic offline replay's retained program output. No
synthetic TCP transitions, process attribution or packet exports are introduced.
See the [source-bound runtime batch proof](fixtures/gewyvern_capture_runtime_linux_x86_64_20261111.json).

The physical x86_64 host completed the same five-phase campaign under a two-core,
512-MiB, 32-task cgroup limit. The two sixty-second unpaused windows delivered
and analyzed 499,997 and 495,389 events: 8,242 and 8,170 events/s, including final
drain time. They rejected 3,648,228 and 3,625,081 observations, respectively,
or 87.95%/87.98% of selected response events. Both windows are explicitly
incomplete. Mean kernel-to-batch-analysis completion latency was 957/966 ms,
with maxima of 1.273/1.272 seconds. All reported percentiles overflow the
100-ms histogram range and remain null, not zero or a guessed percentile.

The deliberate two-second consumer pause produced 686,270 measured output
failures; 73,245 measured events still reached analysis after resumption.
Across all three capture phases, including warmup, 1,139,136 received facts
were ingested and analyzed in 4,455 bounded batches, with nine representative
batch reanalyses passing. Kernel totals reconcile with deliveries plus output
failures. Observed sequence gaps are a lower bound: rejected trailing events
need not have a later received sequence to reveal their gap.

This is a **performance gap, not lossless runtime acceptance**. The runtime
campaign's no-capture mean was 104,175 requests/s. A subsequent same-build
observer-only campaign still delivered 9,346,144 measured events without loss
in its two unpaused windows, averaging 77,869 events/s with p99 of 9 us. Its own
baseline was 97,665 requests/s, so cross-campaign raw rates are not a controlled
isolated-cost comparison. The large runtime-path deficit warrants profiling
conversion, ingestion and analysis separately; simply enlarging the ring would
not resolve sustained excess production.

Normal runtime-phase RSS peaks were 25.39/42.97 MiB. Within each window RSS grew
608/888 KiB; the larger inter-phase increase includes shared allocator and
reporting state and is not evidence of long-duration leak freedom. No transport,
byte-integrity or cgroup memory-limit/OOM error occurred. The build optimized the
project package at level 2 while dependencies used the dev profile, not full
release. CPU includes clients, servers, observer and runtime, not isolated
analysis cost. Both campaigns passed fresh post-pressure HTTP capture/replay.

The independent-batch adapter does not preserve cross-batch session context,
persist the full stream or establish generic-adapter offline replay. Source
and report hashes distinguish this diagnostic result from production claims;
no release gate or status-tensor maturity is promoted.

Related Linux regressions passed 739 unique tests, including explicit Unix/TCP
input cases; seven existing performance benchmarks were not run. Strict Clippy
including test targets and formatting checks passed. Measurement source hashes
and the final equivalent modulo-to-`is_multiple_of` lint adjustment are recorded
separately. Owned BPF resources and temporary build/check containers were removed;
the seven unrelated running service-container IDs were unchanged. Administrative
credentials were invalidated and the session closed. macOS received 468 KiB of
reports, with no local build or image pull.

## `2026-11-11` Continuous Ring Delivery And Consumer Backpressure

The opt-in `linux-tcp-capture --capture-stream` backend now uses the same BPF
packet parser with a fixed 1-MiB ring, 128-byte records and a Rust/libbpf reader
limited to 512 callbacks per batch. No unbounded queue is introduced. Ring
output failure rejects the new observation without blocking network traffic;
malformed selected packets have a separate counter. The previous 128-slot
snapshot mode remains an explicit control. See the [source-bound continuous-delivery proof](fixtures/gewyvern_capture_stream_linux_x86_64_20261111.json).

On the same physical x86_64 kernel `7.0.0-42-generic`, two sixty-second normal
windows delivered 5,265,737 and 5,347,700 checked response events with zero
observed output failures or echo-sequence gaps. Both complete observations
include their warmup. Normal measured delivery averaged 88,424 events/s versus
104,499 requests/s without capture: 15.38% lower throughput under the same
two-core quota, including reader, byte checking and timestamp/histogram work.
These costs cannot be replaced with the previous discard-only 1.56% comparison.
Kernel-to-Rust delivery p50/p95/p99 upper bounds were 4/5/7 us in both windows;
maximum delivery latency was 24.39/21.02 ms, with CPU quota throttling visible.

Pausing the drained consumer for 2.0002 seconds produced exactly 199,283 output
failures and the same number of observed echo-sequence gaps. The ten-second
window still delivered 716,487 events after resuming. Its observation remains
incomplete, not silently repaired. All 7,710 buffered old records exceeded the
100-ms histogram range; the maximum was about two seconds and p99 is explicitly
null/overflow, never zero. Counts reconcile as `received + dropped == total`.
The following normal window was complete again, demonstrating renewed production
and consumption, not just draining the original buffer.

Normal-stream sampled RSS peaks were 9.88/13.54 MiB; these include the shared
test process, mapping, allocator and metric storage, not isolated collector cost.
No transport/byte-integrity error or cgroup memory-limit/OOM event occurred.
Owned links, pins and cgroups were removed. Fresh split-prefix replay, the old
loss/overflow/error/abort/SIGKILL campaign, loopback and Juice Shop paths passed.

Related Linux regressions passed 735 unique tests, including explicit Unix/TCP
socket-input cases; seven existing performance benchmarks were not run. Strict
Clippy including test targets and formatting checks passed. Tested source hashes
match the local worktree. The seven unrelated running service-container IDs were
unchanged, temporary build/check containers were removed and the administrative
session was closed. macOS received only 480 KiB of reports, with no build or image pull.

This proves a bounded kernel-to-Rust delivery path and explicit slow-consumer
loss accounting on one host. The sink is an echo-integrity/latency observer,
not full `RuntimeSession` ledger ingestion or durable storage. Arbitrary-network
retransmission handling, open-loop tails, long-duration leak freedom and
supervisor/host-crash recovery remain outside this proof. No release gate or
status-tensor maturity is promoted.

## `2026-11-11` Sustained Capacity-Exhausted Capture Pressure

The physical Ubuntu x86_64 host (`7.0.0-42-generic`, Ryzen 7 7735H) completed
baseline/capture/capture/baseline phases with sixty measured seconds each and
two seconds of warmup. Two owned loopback clients exchanged checked 64-byte
requests and echoes, one request in flight per client. The native test cgroup
was capped at two CPU cores of quota, 512 MiB and 32 tasks. No build or regression
suite ran concurrently. See the [full metric definitions and compact proof](fixtures/gewyvern_capture_load_linux_x86_64_20261111.json).

Across 24,803,972 measured requests, average baseline throughput was 104,159/s
and capacity-exhausted capture throughput was 102,537/s, a 1.56% reduction in
this experiment. Baseline repeat drift was +0.044%; capture repeat drift was
-0.190%. Baseline RTT p99 upper bounds were 26/27 us, capture p99 was 27/27 us;
all p50/p95 values were 17/22 us. Worst RTT was 15.74 ms. Both modes used roughly
200% of one CPU core and were quota-throttled, so the numbers are quota-bound,
closed-loop application RTTs, not isolated BPF cost or open-loop tail bounds.

The snapshot sampler used in this comparison has 128 slots and no
continuous consumer. Both maps filled during warmup. The measured capture
windows observed and budget-rejected exactly 6,158,248 and 6,146,484 response
events, respectively. Measured rejection was 100%; including warmup it was
approximately 99.99799%. This is **evidence rejection, not network packet loss**.
Counters reconciled with completed requests, no incomplete packet export was
published, and there were no transport or byte-integrity errors. It would be
incorrect to advertise these results as lossless sustained capture throughput.

Sampled process RSS peaks were 6.57/7.32/8.93/9.81 MiB in phase order. The first
capture cgroup charged up to 47.0 MiB, versus 3.32 MiB on the second capture;
cgroup charges include setup/cache/kernel state and are not process RSS. All
phases share one process and retained reporting/allocator state, so neither
the inter-phase RSS difference nor cgroup charge is a clean collector-only
memory overhead measurement. No cgroup memory limit/OOM event occurred. These
short samples do not establish long-duration leak freedom.

Both capture links and every owned pin/cgroup were removed. A fresh real split
HTTP-prefix/body-decoy capture and deterministic replay passed after pressure.
At this snapshot-only stage, continuous drain/backpressure and capture delivery
latency were unproven; the continuous-ring campaign above now covers that bounded
path. Complete runtime ingest performance remains unproven, and status-tensor
maturity is not promoted.

Related Linux regressions passed 732 unique tests, including the explicit Unix
and TCP socket-input cases. Strict Clippy including test targets and formatting
checks passed; seven existing performance benchmarks were not run and are not
counted as passes. The seven unrelated service-container IDs were unchanged.
Temporary build/check containers were removed, the administrative session was
closed, and macOS received only 152 KiB of reports with no local build or image pull.

## `2026-11-04` Capture Loss, Limits And Worker Recovery

The native `linux-tcp-capture --capture-recovery` campaign passed repeated runs
on the physical Ubuntu x86_64 host, kernel `7.0.0-42-generic`. Exactly 128 packet
events exported and replayed successfully. Packet overflow recorded 131 events
with three dropped; TCP-state overflow recorded 160 with 32 dropped. A two-entry
map produced real update failures (five observed, three dropped), while deleting
one detached snapshot entry produced four entries for five counted events and
zero reported drops. All incomplete cases were rejected before packet export.
See the [bounded proof and report hashes](fixtures/gewyvern_capture_recovery_linux_x86_64_20261104.json).

This fixed a lifecycle gap: the tracepoint link was persistently pinned, unlike
the already fd-owned packet link. After setup it is now fd-owned and unpinned.
Owned worker error returns, SIGABRT and SIGKILL all detached both exact kernel
link IDs. Normal error unwinding left no directories; abort/kill each left two
pin directories and an empty cgroup, which the supervisor removed only after
waiting for its child and matching recorded device/inode identities. No core
dump or successful worker export was produced. Fresh capture/replay passed after
each exit and after the loss sequence.

The first trial did not exceed the TCP event budget: 80 connect attempts yielded
80 selected states on this kernel. Increasing to 160 attempts supplied a real
overflow; no counter was fabricated. Failed trial logs remain on Linux. The
campaign sends only owned loopback traffic; it never drops network packets,
terminates unrelated processes or sweeps old resources by name prefix.

This does not prove sustained-load performance, production ring-buffer loss,
supervisor death, host reboot, the pre-ready autoattach/fd-adoption window or a
multi-host/kernel matrix. Trust remains `unverified-local`; no release gate or
status-tensor maturity is promoted.

Three full campaigns passed, including one on final sources. The original
loopback, split-prefix/body-decoy and owned Juice Shop paths also passed again.
Relevant Linux regressions passed 730 unique tests; seven performance benchmarks
were not run. Strict Clippy including test targets, formatting and diff checks
passed. The first documentation-link check preceded creation of the new proof
file and failed; the completed documentation shelf passed on rerun. No owned
pins/cgroups, target container/network or temporary build/check container remained.
The seven unrelated running container IDs were unchanged. macOS received only
520 KiB of small reports and no build artifacts.

## `2026-11-04` Split Prefix And Body Decoy Boundaries

The Docker-free `linux-tcp-capture --http-boundaries` scenario passed repeated
physical-Linux runs. Each captured ten TCP state events and six packet snapshots
(two SYN-ACK origins plus four payload pieces), with zero reported loss and no
unselected socket events. One HTTP 403 status prefix was deliberately split
across two observed packets; a separate HTTP 200 response body began with a fake
`HTTP/1.1 500` prefix. See the [real boundary sample and proof](fixtures/gewyvern_http_boundaries_linux_x86_64_20261104.json).

This exposed a real limitation of the prior packet-local rules: they produced
a 500 candidate from the body and missed the split 403. Those rules now explicitly
label packet candidates, rather than implying a verified response status.
Separate derived evidence assembles only the first thirteen bytes relative to
the peer's captured SYN-ACK sequence plus one. It correctly reports 403 and 200,
keeps anchor/payload fact IDs, and reproduces the result after export replay.
The original packet facts, lengths, sequence numbers and boundaries remain intact.

Unit regressions cover reordered pieces, byte-identical retransmissions,
32-bit sequence wrap, gaps, conflicting overlaps, missing/conflicting origins,
and a zero-length cross-check that otherwise could vacuously pass on a SYN-ACK.
Older samples without a handshake origin remain readable but cannot be silently
promoted to origin-verified results. The container summary moves to schema 2,
separating packet candidates from verified first-response-prefix statuses.
Relevant Linux regressions passed 727 unique tests; seven performance benchmarks
were not run. Strict Clippy including test targets and formatting checks passed.

This is not general TCP reassembly or a complete HTTP parser. Later and interim
responses, Fast Open, TLS, body semantics and vulnerability verdicts are outside
the proof. The unit cases for reordering/retransmission are not claimed as live
network fault injection. All live work stayed on Linux; no release gate or tensor
maturity was promoted. The original loopback and owned Juice Shop paths remain
regression targets, with client I/O separate from kernel-derived evidence.

## `2026-11-04` Kernel Payload Prefix To Gewylang Classification

The container capture path now passed three physical-host runs with an actual
`cgroup_skb/ingress` BPF sampler: ten TCP state events and two nonempty packet
snapshots per run, two reconstructed flows, and zero reported capture loss.
Validation-only Gewylang rules identified HTTP/1.1 403 and 500 from the captured
bytes. Their socket cookies and prefixes matched independently saved client
responses; the unselected HTTP 200 request remained absent. Facts, flows,
reasons, program flows and protocol IR replayed identically. The original
loopback mode also passed. See the [real prefix sample and proof hashes](fixtures/gewyvern_packet_capture_linux_x86_64_20261104.json).

The first attempt exposed a verifier-visible zero-length read bound after LLVM
folding. The sampler now explicitly proves a 1..64-byte read before calling the
helper. That error path removed its owned resources too. Packet and TCP counter
files are separate so one capture cannot overwrite the other's evidence.
Final-source Linux regressions passed 721 unique tests, including two explicitly
enabled socket tests; seven performance benchmarks were not run. Strict Clippy
including test targets and format checks passed. No macOS build or image pull
was used, and no release gate or tensor maturity was promoted.

This remains a bounded validation backend: only two owned private-endpoint
client sockets, ingress IPv4/TCP payloads, 128 events and 64 bytes per prefix.
Every BPF path allows traffic without modification. A temporary cgroup is
removed after restoring the validator's original placement, and the BPF link
is fd-owned. No unrelated process is moved. The sampler records real TCP header
fields; network namespace comes from the correlated kernel TCP state, not a
fabricated PID. Kernel skb snapshots do not imply on-wire packet boundaries.
There is no TCP stream reassembly, TLS decryption, vulnerability verdict,
server-socket attribution or general live-CLI claim. Trust is `unverified-local`.

## `2026-11-04` Container Request To TCP Flow Correlation

The fixed `linux-tcp-capture --juice-shop` mode passed three independent runs on
the same physical Ubuntu `7.0.0-42-generic` host. Each preserved file-guard HTTP
403 and SQL-error HTTP 500 responses, matched ten kernel TCP state events to two
client sockets and reconstructed flows, and excluded a third HTTP 200 request.
Every run reported zero dropped events and replayed identical facts, flows and
reasons. The [real sample and hashes](fixtures/gewyvern_container_capture_linux_x86_64_20261104.json)
retain the cookie/tuple/netns/time/fact-ID joins. The original loopback mode also
passed again after sharing the explicit address-scoped capture backend.
Relevant Linux regressions passed 707 unique tests, including explicitly enabled
TCP/Unix input tests, and targeted strict Clippy passed. Seven performance
benchmarks were not run. The code and raw sample hashes were cross-checked;
only small reports/HTTP evidence were copied to
`target/validation/container-capture-20261104/` on macOS, with no local build.

The first attempt stopped when this Docker internal network did not publish the
requested host port. Isolation was retained: the final path publishes no ports
and connects only to the inspected target IP from its owned bridge gateway.
The second attempt caught chunked HTTP error responses; bounded, strict chunk
framing is now supported, and raw client responses are saved before parsing.
Failed-run logs are retained. The dedicated container, network and BPF pins
were removed on success and both error paths; unrelated services were not used.

The client observations are kept in a separate correlation report. No HTTP
packet facts or PID lineage are synthesized into the kernel export. This is a
bounded client-to-container evidence chain, not BPF payload classification or
vulnerability detection. Sustained load, server-side socket attribution and
multi-host/kernel acceptance remain unproven; trust stays `unverified-local`.

## `2026-11-04` Real TCP Capture And Replay

The same physical Ubuntu x86_64 host (`7.0.0-42-generic`) passed three independent
`linux-tcp-capture` runs: each recorded seven real kernel TCP state events,
reconstructed two owned flows, reported zero dropped events and excluded the
unselected decoy socket. Established, half-closed and closed snapshots replayed
with identical facts, flows and reasons. See the
[captured sample and proof hashes](fixtures/gewyvern_tcp_capture_linux_x86_64_20261104.json).

This uncovered and fixed two product bugs:

- Flow reconstruction treated TCP state numbers as ordered liveness. In
  particular `CLOSE_WAIT` was prematurely terminated. Terminal states are now
  explicit; reconnecting on the same socket clears stale termination evidence.
- TCP fact export omitted both IP addresses and replay silently replaced them
  with zero. New exports preserve all 16 address bytes. Legacy files without
  addresses remain readable; present but malformed address arrays are rejected.

The initial live runs also exposed the host bpftool's nested BTF output format;
the adapter now handles it without guessing untyped raw bytes. Failed-run logs
are retained, and run-owned BPF pins were cleaned on failure as well as success.
Compilation and tests stayed on Linux; no macOS build or image pull was needed.
The final relevant regression set passed 679 unique tests, including explicitly
enabled TCP/Unix input tests, plus targeted strict Clippy. Seven performance
benchmarks were not run and are not counted as passes. Only small reports and
capture JSON were copied to `target/validation/tcp-capture-20261104/` on macOS.

This proves the bounded owned-socket kernel-event-to-runtime path. It does not
prove HTTP payload analysis, vulnerability detection, PID lineage, general live
CLI capture or container-target request correlation. The HTTP 204 was observed
by the test client, not inferred by Gewyvern. Trust stays `unverified-local` and
no status-tensor maturity or release gate was promoted.

## `2026-11-04` Post-Split Linux Field Lab

The current working tree passed its first post-split core validation campaign on
one physical Ubuntu x86_64 host running `7.0.0-42-generic`. Compilation, regression
tests and process-level scenarios ran in a resource-bounded Rust 1.98.0 container
on that host; actual BPF attachment ran on the host itself. No macOS compilation
or image pull was used. See the [bounded proof record](fixtures/gewyvern_field_lab_linux_x86_64_20261104.json).

- All 361 protocol packages passed registry validation; high-frequency/mixed-flow,
  debugger-cross, runtime-operator, runtime-lifecycle and socket/full-scan field
  smoke passed. The latter runtime suites include malformed-input recovery.
- 138 relevant unit/integration tests and targeted strict Clippy passed on Linux.
- Cached Juice Shop, FTP and LDAP targets preserved HTTP 403/SQL-error 500, FTP
  530/server denial, and LDAP 49/server bind-denial evidence respectively. Each
  target also passed tracepoint, kprobe and dedicated-interface TC attach checks.
- The lab entrypoints now require an explicit existing `GEWY_CONTAINER_TC_DEVICE`
  before Docker effects. A run-owned dummy interface replaced automatic primary
  route selection; missing targets were rejected by all three entrypoints.
- Target containers and the dummy interface were removed. Existing non-lab
  containers, the default route and non-transient qdisc state were unchanged.

Initial isolated-container failures came from missing cached test dependencies
and a non-writable default HOME. Fetching the locked dependencies and supplying
a dedicated writable HOME resolved them. The cached Rust image lacked Clippy;
it was installed in a temporary checking container, not on macOS or the host.
The host's default sudo-rs rejected a legacy sudoers wildcard rule; this run used
the existing password-authenticated `sudo.ws`, without changing system policy.

This proves target-side evidence preservation plus same-host attach capability,
not direct vulnerability classification or correlation of the target requests
into Gewyvern's eBPF-derived session output. `resilience-roundtrip` generated
helpers only and is not counted as live fault injection. Sustained-load/loss/tail
latency measurements, full external-engine fault injection and a second host or
kernel remain open. No status-tensor maturity or release gate was promoted.

Full evidence and build caches remain in the remote dedicated Gewyvern workspace;
only small top-level reports were copied to `target/validation/field-lab-20261104/`.
The pinned target image digests in the proof make the cached lab reproducible
without relying on a future `latest` tag.

## `2026-07-18` Linux Publication-Hardening Finding

A physical Ubuntu `6.17.0-35-generic` run exposed a Linux-only ownership drift
in the eBPF loader that macOS compilation could not see. After changing the
probe API to borrow dynamic plan strings and own only emitted failure evidence,
the complete remote validation passed: Linux x86_64 release build, atomic
DEB/RPM publication, strict manifest verification, package/runtime smoke, and
tracepoint, kprobe, and tc eBPF attach. The retained failed workspace was
removed after the successful rerun; synchronized evidence remains under
`target/validation/remote-linux-host-validation`.

The follow-up added a remote Linux all-target compile phase. Its first run
showed that an unanchored rsync `tests/` exclusion had removed Etragon's nested
inline-test module; anchoring the exclusion to the repository root preserved
nested crate/application tests without syncing the root integration shelf. The
corrected physical-host run passed the compile phase and the complete release
path, and the temporary failed workspace was removed.

Because the root integration shelf is only about 1.6 MiB and contains the
Linux-only eBPF tests, it is now synchronized as well. The first complete
compile found three stale borrowed-string assignments in `linux_smoke_tdd`;
after converting them to owned IDs, the full root and nested test targets plus
the release/package/runtime/eBPF path passed on the physical host.

Remote preflight evidence now also pins bounded first-line toolchain identity.
The verified host reported Rust/Cargo `1.95.0`, dpkg-deb `1.22.6`, and
RPM/rpmbuild `4.18.2`; the same values are retained in the latest eBPF history
entry so later failures can be compared against environment drift.

Strict shared parsing of remote preflight, artifact, and eBPF key/value evidence
then exposed that human attach summaries had been mixed into the eBPF stdout
protocol. Redirecting those summaries to stderr preserved operator visibility
while making stdout machine-only. The physical-host rerun passed with a clean
five-entry history and five successful eBPF outcomes.

Remote build, package-smoke, and runtime-smoke timings are now mandatory strict
evidence rather than best-effort files. The physical run produced complete,
canonically ordered finite timing sets for all core phases (with unpack refresh
remaining the only optional cache phase) and completed with six consecutive
successful eBPF outcomes.

## `2026-07-10` Findings Inherited From `1.0.0`

### 1. The Default Release Gate Now Passes As One Routine

The current release-style entrypoint now passes in one run:

- `cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build`

That green path includes:

- packaged release validation in default `deb+rpm` mode
- the three-module stack smoke
- pathological container validation

This is stronger than a collection of isolated green commands because it proves
the current orchestration order also holds up.

The current release-facing artifact trail is also easier to preserve now
because each successful `release-gate` run refreshes:

- `target/validation/release-gate-artifacts.json`
- `target/validation/release-gate-artifacts.txt`

Those two files are the compact index of which release shelves were present at
the time, including optional high-signal evidence such as
`target/validation/juice-shop-container`.

### 2. Packaged Linux Validation Is Green Across Both Package Families

The current packaged release routine is green in default mode through:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

That means the active tree's native artifacts are usable not only as local
build outputs, but as real installed runtime inputs across both `deb` and
`rpm` flows.

### 3. The Multi-Project Stack Now Holds Up Under Resilience Checks

The current Docker collaboration smoke now passes through:

- two `gewyvern` runtimes
- one nearby `etragon` sidecar
- one `leserpent` control plane

The observed green outputs include:

- `etragon-status-ok`
- `etragon-output-ok`
- `summary-ok`
- `runtimes-ok`
- `gw-a-resilience-ok`
- `gw-b-resilience-ok`
- `gw-b-health-degraded-ok`
- `gw-b-resilience-degraded-ok`

This is currently one of the clearest signals that the monorepo collaboration
story is not only documented but operational.

### 4. Pathological Ingest Currently Degrades And Recovers Correctly

The current pathological container gate now passes with all intended bad-input
classes:

- truncated JSON
- empty disconnects
- slow-drip incomplete JSON
- oversized fact lines

The important current finding is not merely that the runtime "survived". It is
that degraded health, degraded resilience, post-fault analysis, and log
evidence all remain observable after the bad clients run.

### 5. Dependency Vulnerability Checks Are Currently Clean

The current release-ready security checks are green for:

- Rust via `cargo audit`
- Leserpent NuGet packages via
  `dotnet list apps/leserpent/src/Leserpent/Leserpent.csproj package --vulnerable`
- Leserpent frontend dependencies via `npm audit`

That closes one of the last release-checklist items that should not remain
implicit before a `1.0` push.

### 6. A Practical Linux Target-Lab Shelf Now Exists

The current Linux host validation work now also has one practical target-side
lab shelf through:

- `cargo run --quiet --bin gewyvern_validate -- juice-shop-container-validation`

The observed green run on `kyuubiki-lab` preserved:

- a file-guard style `403` response carrying
  `Only .md and .pdf files are allowed!`
- a SQL-style `500` response carrying `SQLITE_ERROR: incomplete input`
- same-host `linux-attach-smoke`
- same-host `linux-kprobe-smoke`
- same-host `linux-tc-smoke`

This matters because it is stronger than a synthetic protocol demo while still
being honest about the current product shape:

- `gewyvern` preserved suspicious target evidence
- Linux attach proof still held on the same host
- the result is not yet a claim of direct web-vulnerability classification

## Historical Stable Findings From `0.15.x`

### 1. Registry Validation Is Fully Green

At that point, the built-in protocol registry contained `90` package entries,
and the entire scanned registry passed:

- machine-facing `gewyc envelope --json` validation
- `gewyvern --scan-all --json --summary-only`

This means the current stable protocol shelf is no longer drifting at the
compiler/package level.

### 2. `0.15.0` Native Artifacts Drove The Validation Path

The packaged container path is no longer relying on stale historical artifacts.

Fresh `0.15.0` native packages were rebuilt from the then-current source tree and
then used as the input for the packaged validation chain:

- `target/packages/gewyvern_0.15.0-1_arm64.deb`
- `target/packages/rpm/gewyvern-0.15.0-1.aarch64.rpm`

This matters because an earlier validation pass could still succeed while
quietly exercising an older `0.10.0` package set.

That ambiguity was removed for the `0.15.x` line and remains useful context
for later release validation.

### 3. Packaged Linux Protocol Support Works After Real Install

Both native package families now pass packaged protocol validation inside clean
containers:

- Debian-family install via `.deb`
- RPM-family install via `.rpm`

The packaged validation path now covers:

- `DNS`
- `HTTP`
- `TLS`
- `HTTP/3`
- `QUIC`
- `SSH`
- `SOCKS5`
- `MySQL`
- `PostgreSQL`
- `SMTP`
- `LDAP`
- packaged `--scan-all`

This matters because it confirms that installed asset lookup, protocol registry
discovery, and packaged CLI behavior are working outside the development tree.

### 4. Packaged Standalone Runtime Works In Clean Linux Containers

Installed `.deb` and `.rpm` packages now pass real packaged runtime validation:

- packaged `--serve`
- packaged socket ingest
- packaged `/health`
- packaged `/v1/latest/summary.json`
- packaged `/v1/latest/analysis.json`
- packaged `/v1/latest/export.json`

Malformed ingest was also exercised without killing the packaged service loop.

That gives us a stronger signal than unit tests alone that the standalone
runtime shape survives real install workflows.

### 5. High-Value Operator Paths Already Look Conservative And Coherent

Packaged operator-path validation in clean Linux containers now covers:

- `DNS -> QUIC -> HTTP/3`
- `DNS -> TLS -> HTTPS CONNECT`
- `DNS -> SOCKS5 -> HTTPS CONNECT`
- `DNS -> TLS -> Postgres`
- `DNS -> SMTP`

The current important observation is not that all of these paths land in strong
final diagnosis.

The important observation is that they land in coherent, non-wildly-drifting
states:

- DNS remains a conservative advisory path
- TLS and HTTPS CONNECT remain healthy-but-advisory paths
- QUIC, SOCKS5 auth, PostgreSQL query, and SMTP session remain
  `missing_transition` paths with `collect_more_runtime_evidence`

That is a good prelaunch shape for a standalone debugger: the runtime is
preferring stable conservatism over premature collapse.

### 6. The Release Validation Path Now Holds Up Under Default `deb+rpm` Mode

The release-style compatibility layer exposed one real scripting bug during
this validation cycle:

- `cargo run --quiet --bin gewyvern_validate -- release-container-check`
- `cargo run --quiet --bin gewyvern_validate -- container-validation-summary`

In default `deb+rpm` mode, the underlying wrapper path could trip `set -u` because it
expanded an empty `mode_args` array directly.

That is now fixed, so the native release path can be used as a real release
entrypoint instead of only the explicit `--deb` / `--rpm` submodes.

### 7. A Release-Style Packaged Linux Checklist Already Passes

The current release-style container validation path now passes as one
deliberate checklist run across both package families:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

This matters because it is stronger than saying “individual scripts look good”.

It means the packaged Linux path can already be exercised as one release-minded
routine, not only as disconnected local checks.

### 8. HTTP/3 Validation Expectations Are Now Aligned With Current Guidance

This validation cycle also exposed one important expectation drift:

- the current `http3 request` packaged path now lands in
  `operator_guidance_action = "safe_to_escalate_protocol_signal"`
- the older container assertions still expected
  `operator_guidance_action = "manual_review"`

The implementation and the focused HTTP/3 tests already agreed with the newer
behavior. What was stale was the packaged validation expectation.

That mismatch is now corrected, so the container suite reflects the current
diagnosis semantics instead of an older advisory-only assumption.

### 9. The Three-Module Stack Already Works In Containers

The current collaboration topology:

- `leserpent -> many gewyvern`
- `etragon <-> one nearby gewyvern`

now passes a real Docker smoke through:

- two `gewyvern` runtimes
- one nearby `etragon` sidecar
- one `leserpent` control plane

The stack smoke currently verifies:

- both `gewyvern` runtimes publish latest snapshot state
- `etragon` publishes additive output with augmentations
- `leserpent` can register both runtimes
- fleet summary reflects the paired sidecar correctly
- runtime detail reflects both plain and sidecar-backed nodes correctly

This is one of the highest-signal findings in the current line, because it
shows that the collaboration model is not only documented; it already works in
an isolated container environment close to the intended topology.

## Current Conservative Findings

### 1. Some “Denied” Demo Entries Do Not Yet Produce Strong Denial Semantics

Current packaged and local synthetic demo validation shows that entries such as:

- `socks5 auth-denied`
- `socks5 auth-connect-denied`
- `smtp rcpt-denied`
- `smtp data-denied`

still resolve to:

- `primary_failure_basis = "missing_transition"`
- a DNS/setup-oriented failure posture
- `operator_guidance_action = "collect_more_runtime_evidence"`

instead of a stronger denial-style diagnosis.

For the current line, this is treated as a conservative result rather than a
bug, because it avoids over-claiming on synthetic evidence that does not yet
drive the path far enough.

It does mean that these demo entries should currently be read as:

- “do not overtrust strong denial semantics here yet”

rather than:

- “this path is already a rich negative diagnosis oracle”

### 2. Packaged Negative-Path Validation Currently Focuses On Non-Overcollapse

The strongest current negative-path guarantee in packaged validation is:

- denial-style demo entries do not over-collapse into stronger claims when the
  evidence is still only setup-incomplete

That is useful and intentional, but it is not the same thing as saying:

- every negative demo path already yields a strong, human-like final diagnosis

## Strict Remote Evidence Consumption

The `2.5.x` validation CLI now consumes synchronized Linux-host evidence with
the same fail-closed posture used by the producer. Key-value evidence has a
shared bounded unique-key codec, phase timings require complete finite schemas,
and local history JSON and recent-history text have explicit file and record
limits. Human and JSON summaries reuse one parsed value, so they cannot disagree
because of a second read.

The latest physical Linux package, runtime, and eBPF run completed successfully
before this consumer-only hardening and extended the retained eBPF trend to
`7/7`. The hardened consumer is additionally covered by the real summary
fixture and malformed, duplicate, unknown, oversized, overlong, and
control-character negative tests.

## Non-Vacuous Leserpent Security Proof

The Leserpent parity/recovery shelf now executes the .NET control-plane security
project as a first-class proof suite instead of relying on a manual release
command. Its parser requires one internally consistent nonzero `Passed!`
summary and locked, proof-local build artifacts. Current macOS arm64 and
physical Linux x86_64 runs both completed 13 suites with 231 observed tests and
155 invariants; the xUnit contribution was 72 passed, zero failed, and zero
skipped. The Linux summary binds this result to kernel `6.17.0-35-generic`,
Rust/Cargo `1.95.0`, and .NET `10.0.109`. Proof-local .NET artifacts were
removed after evidence capture.

The shelf now removes its known summary, index, and suite logs before starting.
A failed rerun therefore cannot leave an earlier green summary available as if
it described the attempted run.

## Practical Read Of The Current Line

The current `2.5.x` line inherits strength in these areas:

- protocol/package shelf is stable
- current tree-native artifacts are the ones being exercised
- packaged standalone runtime works
- packaged high-frequency protocol families work
- packaged operator paths stay conservative and coherent
- packaged Linux release-style validation can run as one checklist in default
  `deb+rpm` mode
- the current three-module collaboration topology already works in Docker
- pathological ingest validation currently preserves degraded-but-live runtime
  behavior
- dependency vulnerability checks are currently green across Rust, .NET, and
  frontend lockfiles

The current line should still be read cautiously in these ways:

- some synthetic “denied” entries are still setup-shaped rather than richly
  denial-shaped
- the runtime is currently more trustworthy as a conservative diagnosis engine
  than as an aggressively collapsing one

That is still a good prelaunch posture.

It is much safer for first release validation than a system that appears more
confident than its evidence really supports.

## Visible Postlaunch Follow-Ups

These items are already visible, but they should stay out of the prelaunch
line unless field validation shows drift rather than conservatism.

### 1. Richer Negative-Path Semantics For Denied Demo Entries

Observed today:

- `socks5 auth-denied`
- `socks5 auth-connect-denied`
- `smtp rcpt-denied`
- `smtp data-denied`

currently stay in a setup-shaped conservative posture:

- `primary_failure_basis = "missing_transition"`
- DNS/setup-oriented stage selection
- `operator_guidance_action = "collect_more_runtime_evidence"`

This is not a prelaunch blocker because it avoids false strong conclusions.
It is still worth revisiting after launch so these entries can eventually carry
stronger denial semantics when the synthetic path genuinely drives far enough.

### 2. Stronger Failure-Oriented Packaged Operator Paths

Observed today:

- packaged operator-path validation is strong on conservative mixed/setup paths
- it is still thinner on richly denial-shaped packaged paths

This is not a prelaunch blocker because the more important guarantee is that
packaged installs behave coherently and do not invent overconfident conclusions.

### 3. Better Separation Between Advisory Paths And Stronger Final Diagnoses

Observed today:

- the runtime is more trustworthy as a conservative diagnosis engine than as an
  aggressively collapsing one
- this is good for launch trust, but it leaves some demo paths more advisory
  than an operator may hope

That is still the right prelaunch posture. After launch, high-value protocol
families can be revisited one by one to decide where stronger final diagnoses
are actually justified by evidence.
