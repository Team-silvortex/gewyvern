use super::summary::built_in_protocol_summary;

#[test]
fn built_in_redis_summary_covers_packaged_family_shape() {
    let summary = built_in_protocol_summary("redis").expect("redis summary should exist");
    let entries = summary
        .entries
        .into_iter()
        .map(|entry| entry.mode)
        .collect::<Vec<_>>();

    for mode in [
        "ping",
        "session",
        "get",
        "set",
        "auth-required",
        "auth-denied",
        "error",
        "wrongtype",
        "busygroup",
        "readonly",
        "noscript",
        "moved",
        "ask",
        "tryagain",
        "loading",
        "crossslot",
        "clusterdown",
        "masterdown",
        "oom",
        "busy",
        "execabort",
        "misconf",
        "publish",
        "subscribe",
        "zadd",
        "xreadgroup",
        "bzpopmax",
        "blmove",
    ] {
        assert!(
            entries.contains(&mode.to_string()),
            "built-in redis summary should contain `{mode}`"
        );
    }
}

#[test]
fn built_in_redis_summary_keeps_high_value_entry_aliases() {
    let summary = built_in_protocol_summary("redis").expect("redis summary should exist");
    let entries = summary.entries;

    let get = entries
        .iter()
        .find(|entry| entry.mode == "get")
        .expect("redis get entry should exist");
    assert!(get.aliases.contains(&"read".to_string()));
    assert!(get.aliases.contains(&"kv-read".to_string()));

    let auth_required = entries
        .iter()
        .find(|entry| entry.mode == "auth-required")
        .expect("redis auth-required entry should exist");
    assert!(auth_required.aliases.contains(&"login-required".to_string()));

    let auth_denied = entries
        .iter()
        .find(|entry| entry.mode == "auth-denied")
        .expect("redis auth-denied entry should exist");
    assert!(auth_denied.aliases.contains(&"wrongpass".to_string()));

    let publish = entries
        .iter()
        .find(|entry| entry.mode == "publish")
        .expect("redis publish entry should exist");
    assert!(publish.aliases.contains(&"pubsub-send".to_string()));

    let wrongtype = entries
        .iter()
        .find(|entry| entry.mode == "wrongtype")
        .expect("redis wrongtype entry should exist");
    assert!(wrongtype.aliases.contains(&"type-conflict".to_string()));

    let busygroup = entries
        .iter()
        .find(|entry| entry.mode == "busygroup")
        .expect("redis busygroup entry should exist");
    assert!(busygroup.aliases.contains(&"stream-group-exists".to_string()));

    let readonly = entries
        .iter()
        .find(|entry| entry.mode == "readonly")
        .expect("redis readonly entry should exist");
    assert!(readonly.aliases.contains(&"replica-write-denied".to_string()));

    let noscript = entries
        .iter()
        .find(|entry| entry.mode == "noscript")
        .expect("redis noscript entry should exist");
    assert!(noscript.aliases.contains(&"script-missing".to_string()));

    let moved = entries
        .iter()
        .find(|entry| entry.mode == "moved")
        .expect("redis moved entry should exist");
    assert!(moved.aliases.contains(&"cluster-redirect".to_string()));

    let ask = entries
        .iter()
        .find(|entry| entry.mode == "ask")
        .expect("redis ask entry should exist");
    assert!(ask.aliases.contains(&"cluster-ask".to_string()));

    let tryagain = entries
        .iter()
        .find(|entry| entry.mode == "tryagain")
        .expect("redis tryagain entry should exist");
    assert!(tryagain.aliases.contains(&"cluster-retry".to_string()));

    let loading = entries
        .iter()
        .find(|entry| entry.mode == "loading")
        .expect("redis loading entry should exist");
    assert!(loading.aliases.contains(&"loading-window".to_string()));

    let crossslot = entries
        .iter()
        .find(|entry| entry.mode == "crossslot")
        .expect("redis crossslot entry should exist");
    assert!(crossslot.aliases.contains(&"multi-key-slot-conflict".to_string()));

    let clusterdown = entries
        .iter()
        .find(|entry| entry.mode == "clusterdown")
        .expect("redis clusterdown entry should exist");
    assert!(clusterdown.aliases.contains(&"cluster-unavailable".to_string()));

    let masterdown = entries
        .iter()
        .find(|entry| entry.mode == "masterdown")
        .expect("redis masterdown entry should exist");
    assert!(masterdown.aliases.contains(&"primary-unavailable".to_string()));

    let oom = entries
        .iter()
        .find(|entry| entry.mode == "oom")
        .expect("redis oom entry should exist");
    assert!(oom.aliases.contains(&"memory-limit".to_string()));

    let busy = entries
        .iter()
        .find(|entry| entry.mode == "busy")
        .expect("redis busy entry should exist");
    assert!(busy.aliases.contains(&"script-busy".to_string()));

    let execabort = entries
        .iter()
        .find(|entry| entry.mode == "execabort")
        .expect("redis execabort entry should exist");
    assert!(execabort.aliases.contains(&"transaction-abort".to_string()));

    let misconf = entries
        .iter()
        .find(|entry| entry.mode == "misconf")
        .expect("redis misconf entry should exist");
    assert!(misconf.aliases.contains(&"persistence-misconfig".to_string()));

    let zadd = entries
        .iter()
        .find(|entry| entry.mode == "zadd")
        .expect("redis zadd entry should exist");
    assert!(zadd.aliases.contains(&"sorted-add".to_string()));

    let xreadgroup = entries
        .iter()
        .find(|entry| entry.mode == "xreadgroup")
        .expect("redis xreadgroup entry should exist");
    assert!(
        xreadgroup
            .aliases
            .contains(&"stream-group-read".to_string())
    );
}

#[test]
fn built_in_mqtt_summary_covers_qos2_and_teardown_entries() {
    let summary = built_in_protocol_summary("mqtt").expect("mqtt summary should exist");
    let entries = summary
        .entries
        .into_iter()
        .map(|entry| entry.mode)
        .collect::<Vec<_>>();

    for mode in [
        "connect",
        "publish",
        "subscribe",
        "disconnect",
        "pubrec",
        "pubrel",
        "pubcomp",
    ] {
        assert!(
            entries.contains(&mode.to_string()),
            "built-in mqtt summary should contain `{mode}`"
        );
    }
}

#[test]
fn built_in_mqtt_summary_keeps_session_pubsub_and_qos2_aliases() {
    let summary = built_in_protocol_summary("mqtt").expect("mqtt summary should exist");
    let entries = summary.entries;

    let connect = entries
        .iter()
        .find(|entry| entry.mode == "connect")
        .expect("mqtt connect entry should exist");
    assert!(connect.aliases.contains(&"session".to_string()));

    let publish = entries
        .iter()
        .find(|entry| entry.mode == "publish")
        .expect("mqtt publish entry should exist");
    assert!(publish.aliases.contains(&"send".to_string()));

    let subscribe = entries
        .iter()
        .find(|entry| entry.mode == "subscribe")
        .expect("mqtt subscribe entry should exist");
    assert!(subscribe.aliases.contains(&"listen".to_string()));

    let pubrel = entries
        .iter()
        .find(|entry| entry.mode == "pubrel")
        .expect("mqtt pubrel entry should exist");
    assert!(pubrel.aliases.contains(&"qos2-release".to_string()));
}

#[test]
fn built_in_amqp_summary_keeps_session_publish_and_consume() {
    let summary = built_in_protocol_summary("amqp").expect("amqp summary should exist");
    let entries = summary
        .entries
        .into_iter()
        .map(|entry| entry.mode)
        .collect::<Vec<_>>();

    for mode in ["start", "publish", "consume", "session"] {
        assert!(
            entries.contains(&mode.to_string()),
            "built-in amqp summary should contain `{mode}`"
        );
    }
}

#[test]
fn built_in_amqp_summary_keeps_handshake_and_delivery_aliases() {
    let summary = built_in_protocol_summary("amqp").expect("amqp summary should exist");
    let entries = summary.entries;

    let start = entries
        .iter()
        .find(|entry| entry.mode == "start")
        .expect("amqp start entry should exist");
    assert!(start.aliases.contains(&"login".to_string()));

    let session = entries
        .iter()
        .find(|entry| entry.mode == "session")
        .expect("amqp session entry should exist");
    assert!(session.aliases.contains(&"connect".to_string()));

    let consume = entries
        .iter()
        .find(|entry| entry.mode == "consume")
        .expect("amqp consume entry should exist");
    assert!(consume.aliases.contains(&"receive".to_string()));
    assert!(consume.aliases.contains(&"deliver".to_string()));
}

#[test]
fn built_in_ftp_summary_keeps_control_and_data_aliases() {
    let summary = built_in_protocol_summary("ftp").expect("ftp summary should exist");
    let entries = summary.entries;

    let session = entries
        .iter()
        .find(|entry| entry.mode == "session")
        .expect("ftp session entry should exist");
    assert!(session.aliases.contains(&"login".to_string()));
    assert!(session.aliases.contains(&"control".to_string()));

    let active_retr = entries
        .iter()
        .find(|entry| entry.mode == "active-retr")
        .expect("ftp active-retr entry should exist");
    assert!(active_retr.aliases.contains(&"active-download".to_string()));
}

#[test]
fn built_in_ldap_summary_keeps_directory_and_sync_aliases() {
    let summary = built_in_protocol_summary("ldap").expect("ldap summary should exist");
    let entries = summary.entries;

    let bind = entries
        .iter()
        .find(|entry| entry.mode == "bind")
        .expect("ldap bind entry should exist");
    assert!(bind.aliases.contains(&"login".to_string()));
    assert!(bind.aliases.contains(&"auth".to_string()));

    let search = entries
        .iter()
        .find(|entry| entry.mode == "search")
        .expect("ldap search entry should exist");
    assert!(search.aliases.contains(&"directory".to_string()));

    let sync = entries
        .iter()
        .find(|entry| entry.mode == "sync")
        .expect("ldap sync entry should exist");
    assert!(sync.aliases.contains(&"replication".to_string()));
}

#[test]
fn built_in_rtsp_summary_keeps_probe_and_stream_aliases() {
    let summary = built_in_protocol_summary("rtsp").expect("rtsp summary should exist");
    let entries = summary.entries;

    let options = entries
        .iter()
        .find(|entry| entry.mode == "options")
        .expect("rtsp options entry should exist");
    assert!(options.aliases.contains(&"probe".to_string()));

    let setup = entries
        .iter()
        .find(|entry| entry.mode == "setup")
        .expect("rtsp setup entry should exist");
    assert!(setup.aliases.contains(&"stream".to_string()));
}

#[test]
fn built_in_snmp_summary_keeps_bulk_read_write_and_notify_aliases() {
    let summary = built_in_protocol_summary("snmp").expect("snmp summary should exist");
    let entries = summary.entries;

    let bulk = entries
        .iter()
        .find(|entry| entry.mode == "bulk")
        .expect("snmp bulk entry should exist");
    assert!(bulk.aliases.contains(&"bulk-walk".to_string()));
    assert!(bulk.aliases.contains(&"table-read".to_string()));

    let get = entries
        .iter()
        .find(|entry| entry.mode == "get")
        .expect("snmp get entry should exist");
    assert!(get.aliases.contains(&"query".to_string()));
    assert!(get.aliases.contains(&"read".to_string()));

    let get_next = entries
        .iter()
        .find(|entry| entry.mode == "get-next")
        .expect("snmp get-next entry should exist");
    assert!(get_next.aliases.contains(&"walk".to_string()));

    let set = entries
        .iter()
        .find(|entry| entry.mode == "set")
        .expect("snmp set entry should exist");
    assert!(set.aliases.contains(&"write".to_string()));
    assert!(set.aliases.contains(&"update".to_string()));

    let trap = entries
        .iter()
        .find(|entry| entry.mode == "trap")
        .expect("snmp trap entry should exist");
    assert!(trap.aliases.contains(&"notify".to_string()));
    assert!(trap.aliases.contains(&"alert".to_string()));

    let inform = entries
        .iter()
        .find(|entry| entry.mode == "inform")
        .expect("snmp inform entry should exist");
    assert!(inform.aliases.contains(&"ack-notify".to_string()));
    assert!(inform.aliases.contains(&"confirm-notify".to_string()));

    let engine_sync = entries
        .iter()
        .find(|entry| entry.mode == "engine-sync")
        .expect("snmp engine-sync entry should exist");
    assert!(engine_sync.aliases.contains(&"engine-discovery".to_string()));
    assert!(engine_sync.aliases.contains(&"report-sync".to_string()));

    let report = entries
        .iter()
        .find(|entry| entry.mode == "report")
        .expect("snmp report entry should exist");
    assert!(report.aliases.contains(&"engine-report".to_string()));
    assert!(report.aliases.contains(&"report-pdu".to_string()));

    let trap_recv = entries
        .iter()
        .find(|entry| entry.mode == "trap-recv")
        .expect("snmp trap-recv entry should exist");
    assert!(trap_recv.aliases.contains(&"listen-trap".to_string()));
    assert!(trap_recv.aliases.contains(&"trap-listener".to_string()));

    let unauthorized = entries
        .iter()
        .find(|entry| entry.mode == "unauthorized")
        .expect("snmp unauthorized entry should exist");
    assert!(unauthorized.aliases.contains(&"auth-failed".to_string()));
    assert!(unauthorized.aliases.contains(&"access-denied".to_string()));

    let v3_auth = entries
        .iter()
        .find(|entry| entry.mode == "v3-auth")
        .expect("snmp v3-auth entry should exist");
    assert!(v3_auth.aliases.contains(&"auth-user".to_string()));
    assert!(v3_auth.aliases.contains(&"auth-session".to_string()));

    let v3_priv = entries
        .iter()
        .find(|entry| entry.mode == "v3-priv")
        .expect("snmp v3-priv entry should exist");
    assert!(v3_priv.aliases.contains(&"private-session".to_string()));
    assert!(v3_priv.aliases.contains(&"encrypted-session".to_string()));
}
