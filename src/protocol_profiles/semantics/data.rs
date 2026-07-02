mod redis;

use super::common::{failure, summary};
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn redis_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    redis::redis_entry_semantics(entry)
}

pub(super) fn mysql_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "connect" => summary(
            "mysql-connect-path",
            "MySQL initial handshake and capability negotiation before authentication",
            Some("HandshakeV10 packet"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth" => summary(
            "mysql-auth-path",
            "MySQL authentication exchange accepted after client handshake response",
            Some("Handshake Response + OK packet"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "query" => summary(
            "mysql-query-path",
            "MySQL COM_QUERY request followed by an OK or result-set response",
            Some("COM_QUERY 0x03 + OK/result"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "session" => summary(
            "mysql-query-session-path",
            "MySQL authenticated session carrying one or more SQL query exchanges",
            Some("auth OK + COM_QUERY"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth-denied" => failure(
            "database authentication rejection during MySQL handshake response evaluation",
            Some("ERR"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "error" => failure(
            "database error response during MySQL query result handling",
            Some("ERR"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => None,
    }
}

pub(super) fn postgres_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "connect" => summary(
            "postgres-connect-path",
            "PostgreSQL startup message and server authentication negotiation",
            Some("StartupMessage + Authentication request"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth" => summary(
            "postgres-auth-path",
            "PostgreSQL password exchange accepted before the session becomes ready",
            Some("PasswordMessage + AuthenticationOk"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "query" => summary(
            "postgres-query-path",
            "PostgreSQL simple query message followed by ReadyForQuery",
            Some("Query message 'Q' + ReadyForQuery 'Z'"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "session" => summary(
            "postgres-query-session-path",
            "PostgreSQL authenticated session carrying simple query exchanges",
            Some("AuthenticationOk + Query + ReadyForQuery"),
            None,
            None,
            Some("protocol_entry_signal"),
        ),
        "auth-denied" => failure(
            "database authentication rejection after PostgreSQL password exchange",
            Some("ErrorResponse"),
            Some("server_denied"),
            Some("access_denied"),
        ),
        "error" => failure(
            "database error frame during PostgreSQL query result handling",
            Some("ErrorResponse"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => None,
    }
}

pub(super) fn mongodb_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    if entry == "query-failure" {
        return failure(
            "legacy MongoDB OP_REPLY QueryFailure response returned by the server",
            Some("OP_REPLY QueryFailure"),
            Some("semantic_error"),
            Some("protocol_error"),
        );
    }

    let (category, operator_focus, typical_signal) = match entry {
        "command" => (
            "mongodb-command-path",
            "MongoDB OP_MSG command sent to a server on the wire protocol",
            Some("OP_MSG opcode 2013"),
        ),
        "reply" => (
            "mongodb-reply-path",
            "MongoDB OP_MSG or legacy OP_REPLY response from the server",
            Some("OP_MSG / OP_REPLY"),
        ),
        "legacy-query" => (
            "mongodb-legacy-query-path",
            "legacy MongoDB OP_QUERY request used by older clients or compatibility paths",
            Some("OP_QUERY opcode 2004"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn cassandra_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "error" => failure(
            "Cassandra native protocol ERROR frame returned by the server",
            Some("ERROR opcode 0x00"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => {
            let (category, operator_focus, typical_signal) = match entry {
                "startup" => (
                    "cassandra-startup-path",
                    "Cassandra native protocol STARTUP frame for cluster session setup",
                    Some("STARTUP opcode 0x01"),
                ),
                "authenticate" => (
                    "cassandra-authenticate-path",
                    "Cassandra native protocol AUTHENTICATE frame requiring client authentication",
                    Some("AUTHENTICATE opcode 0x03"),
                ),
                "query" => (
                    "cassandra-query-path",
                    "Cassandra native protocol QUERY frame carrying a CQL request",
                    Some("QUERY opcode 0x07"),
                ),
                "result" => (
                    "cassandra-result-path",
                    "Cassandra native protocol RESULT frame returned by the server",
                    Some("RESULT opcode 0x08"),
                ),
                _ => return None,
            };
            summary(
                category,
                operator_focus,
                typical_signal,
                None,
                None,
                Some("protocol_entry_signal"),
            )
        }
    }
}

pub(super) fn mssql_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    match entry {
        "error" => failure(
            "SQL Server TDS error token returned in a tabular response",
            Some("ERROR token 0xaa"),
            Some("semantic_error"),
            Some("protocol_error"),
        ),
        _ => {
            let (category, operator_focus, typical_signal) = match entry {
                "prelogin" => (
                    "mssql-prelogin-path",
                    "TDS PRELOGIN packet for SQL Server session setup",
                    Some("packet type 0x12"),
                ),
                "login" => (
                    "mssql-login-path",
                    "TDS LOGIN packet for SQL Server authentication",
                    Some("packet type 0x10"),
                ),
                "query" => (
                    "mssql-query-path",
                    "TDS SQL batch packet sent to SQL Server",
                    Some("packet type 0x01"),
                ),
                "response" => (
                    "mssql-response-path",
                    "TDS tabular response packet returned by SQL Server",
                    Some("packet type 0x04"),
                ),
                "colmetadata" => (
                    "mssql-colmetadata-path",
                    "TDS COLMETADATA token describing the shape of a SQL Server result set",
                    Some("COLMETADATA token 0x81"),
                ),
                "row" => (
                    "mssql-row-path",
                    "TDS row token carrying SQL Server result-row data",
                    Some("ROW/NBCROW token 0xd1/0xd2"),
                ),
                "done" => (
                    "mssql-done-path",
                    "TDS DONE-family token marking response completion or sub-batch completion",
                    Some("DONE/DONEPROC/DONEINPROC token 0xfd/0xfe/0xff"),
                ),
                "envchange" => (
                    "mssql-envchange-path",
                    "TDS ENVCHANGE token marking session environment changes",
                    Some("ENVCHANGE token 0xe3"),
                ),
                _ => return None,
            };
            summary(
                category,
                operator_focus,
                typical_signal,
                None,
                None,
                Some("protocol_entry_signal"),
            )
        }
    }
}

pub(super) fn kafka_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "metadata" => (
            "broker-metadata-path",
            "Kafka broker metadata lookup on TCP port 9092",
            Some("Metadata API key"),
        ),
        "api-versions" => (
            "broker-capability-path",
            "Kafka ApiVersions compatibility negotiation between client and broker",
            Some("ApiVersions API key"),
        ),
        "produce" => (
            "stream-produce-path",
            "Kafka produce request/response against broker topic partitions",
            Some("Produce API key"),
        ),
        "fetch" => (
            "stream-fetch-path",
            "Kafka fetch request/response from broker topic partitions",
            Some("Fetch API key"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn nats_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "connect" => (
            "message-session-path",
            "NATS INFO/CONNECT session setup on TCP port 4222",
            Some("INFO / CONNECT"),
        ),
        "pub" => (
            "message-publish-path",
            "NATS PUB command sending a subject payload",
            Some("PUB"),
        ),
        "sub" => (
            "message-subscribe-path",
            "NATS SUB command and MSG delivery",
            Some("SUB / MSG"),
        ),
        "error" => {
            return failure(
                "NATS server-side protocol or authorization error",
                Some("-ERR"),
                Some("semantic_error"),
                Some("protocol_error"),
            );
        }
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn mqtt_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "connect" => (
            "broker-session-path",
            "MQTT CONNECT request and successful CONNACK response",
            Some("CONNECT / CONNACK"),
        ),
        "connack" => (
            "broker-acknowledgement-path",
            "MQTT broker CONNACK response, including refused connection codes",
            Some("CONNACK"),
        ),
        "publish" => (
            "message-publish-path",
            "MQTT PUBLISH and PUBACK message flow",
            Some("PUBLISH / PUBACK"),
        ),
        "subscribe" => (
            "message-subscribe-path",
            "MQTT SUBSCRIBE and SUBACK message flow",
            Some("SUBSCRIBE / SUBACK"),
        ),
        "disconnect" => (
            "broker-teardown-path",
            "MQTT explicit DISCONNECT teardown",
            Some("DISCONNECT"),
        ),
        "pubrec" | "pubrel" | "pubcomp" => (
            "qos2-continuation-path",
            "MQTT QoS2 publish continuation stage",
            Some("PUBREC / PUBREL / PUBCOMP"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}

pub(super) fn memcached_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "get" => (
            "cache-read-path",
            "Memcached binary GET request and value response",
            Some("GET / VALUE"),
        ),
        "miss" => (
            "cache-miss-path",
            "Memcached binary GET response with NOT_FOUND status",
            Some("NOT_FOUND"),
        ),
        "set" => (
            "cache-write-path",
            "Memcached binary SET request and stored response",
            Some("SET / STORED"),
        ),
        "not-stored" => (
            "cache-not-stored-path",
            "Memcached binary SET response with NOT_STORED status",
            Some("NOT_STORED"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("protocol_entry_signal"),
    )
}
