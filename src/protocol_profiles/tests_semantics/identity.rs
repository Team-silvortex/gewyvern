use super::super::protocol_surface;

#[test]
fn imap_and_pop3_auth_denied_surfaces_expose_machine_readable_entry_semantics() {
    let imap_denied =
        protocol_surface("imap", "auth-denied").expect("imap auth-denied should exist");
    let imap_semantics = imap_denied
        .entry_semantics
        .expect("imap auth-denied should expose semantics");
    assert_eq!(imap_semantics.category, "failure-path");
    assert_eq!(
        imap_semantics.operator_focus,
        "mailbox login rejection after LOGIN credential exchange"
    );
    assert_eq!(imap_semantics.typical_signal.as_deref(), Some("NO"));
    assert_eq!(
        imap_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        imap_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        imap_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let pop3_denied =
        protocol_surface("pop3", "auth-denied").expect("pop3 auth-denied should exist");
    let pop3_semantics = pop3_denied
        .entry_semantics
        .expect("pop3 auth-denied should expose semantics");
    assert_eq!(pop3_semantics.category, "failure-path");
    assert_eq!(
        pop3_semantics.operator_focus,
        "mailbox password rejection after USER/PASS credential exchange"
    );
    assert_eq!(pop3_semantics.typical_signal.as_deref(), Some("-ERR"));
    assert_eq!(
        pop3_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        pop3_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        pop3_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let imap_auth = protocol_surface("imap", "auth").expect("imap auth should exist");
    let imap_auth_semantics = imap_auth
        .entry_semantics
        .expect("imap auth should expose positive mailbox auth semantics");
    assert_eq!(imap_auth_semantics.category, "mailbox-auth-path");

    let pop3_auth = protocol_surface("pop3", "auth").expect("pop3 auth should exist");
    let pop3_auth_semantics = pop3_auth
        .entry_semantics
        .expect("pop3 auth should expose positive mailbox auth semantics");
    assert_eq!(pop3_auth_semantics.category, "mailbox-auth-path");
}

#[test]
fn snmp_and_stun_denial_surfaces_expose_machine_readable_entry_semantics() {
    let unauthorized =
        protocol_surface("snmp", "unauthorized").expect("snmp unauthorized should exist");
    let unauthorized_semantics = unauthorized
        .entry_semantics
        .expect("snmp unauthorized should expose semantics");
    assert_eq!(unauthorized_semantics.category, "failure-path");
    assert_eq!(
        unauthorized_semantics.operator_focus,
        "authorization failure report after SNMPv3 access evaluation"
    );
    assert!(unauthorized_semantics.typical_signal.is_none());
    assert_eq!(
        unauthorized_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        unauthorized_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        unauthorized_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let binding_error =
        protocol_surface("stun", "binding-error").expect("stun binding-error should exist");
    let binding_error_semantics = binding_error
        .entry_semantics
        .expect("stun binding-error should expose semantics");
    assert_eq!(binding_error_semantics.category, "failure-path");
    assert_eq!(
        binding_error_semantics.operator_focus,
        "explicit binding failure response instead of successful reachability confirmation"
    );
    assert!(binding_error_semantics.typical_signal.is_none());
    assert_eq!(
        binding_error_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        binding_error_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        binding_error_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let binding_denied =
        protocol_surface("stun", "binding-denied").expect("stun binding-denied alias should exist");
    let binding_denied_semantics = binding_denied
        .entry_semantics
        .expect("stun binding-denied alias should expose semantics");
    assert_eq!(
        binding_denied_semantics.operator_focus,
        "explicit binding failure response instead of successful reachability confirmation"
    );

    let snmp_bulk = protocol_surface("snmp", "bulk").expect("snmp bulk should exist");
    let snmp_bulk_semantics = snmp_bulk
        .entry_semantics
        .expect("snmp bulk should expose management read semantics");
    assert_eq!(snmp_bulk_semantics.category, "snmp-bulk-read-path");

    let snmp_inform = protocol_surface("snmp", "inform").expect("snmp inform should exist");
    let snmp_inform_semantics = snmp_inform
        .entry_semantics
        .expect("snmp inform should expose acknowledged notification semantics");
    assert_eq!(
        snmp_inform_semantics.category,
        "snmp-acknowledged-notification-path"
    );

    let snmp_v3_priv = protocol_surface("snmp", "v3-priv").expect("snmp v3-priv should exist");
    let snmp_v3_priv_semantics = snmp_v3_priv
        .entry_semantics
        .expect("snmp v3-priv should expose private security semantics");
    assert_eq!(snmp_v3_priv_semantics.category, "snmpv3-private-path");

    let snmp_report = protocol_surface("snmp", "report").expect("snmp report should exist");
    let snmp_report_semantics = snmp_report
        .entry_semantics
        .expect("snmp report should expose result semantics");
    assert_eq!(snmp_report_semantics.category, "snmp-report-path");

    let stun_binding = protocol_surface("stun", "binding").expect("stun binding should exist");
    let stun_binding_semantics = stun_binding
        .entry_semantics
        .expect("stun binding should expose positive NAT binding semantics");
    assert_eq!(stun_binding_semantics.category, "nat-binding-path");
}

#[test]
fn smtp_and_kerberos_failure_surfaces_expose_machine_readable_entry_semantics() {
    let auth_denied =
        protocol_surface("smtp", "auth-denied").expect("smtp auth-denied should exist");
    let auth_denied_semantics = auth_denied
        .entry_semantics
        .expect("smtp auth-denied should expose semantics");
    assert_eq!(auth_denied_semantics.category, "failure-path");
    assert_eq!(
        auth_denied_semantics.operator_focus,
        "smtp authentication rejection after explicit AUTH exchange"
    );
    assert_eq!(auth_denied_semantics.typical_signal.as_deref(), Some("535"));
    assert_eq!(
        auth_denied_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        auth_denied_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        auth_denied_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let rcpt_denied =
        protocol_surface("smtp", "rcpt-denied").expect("smtp rcpt-denied should exist");
    let rcpt_denied_semantics = rcpt_denied
        .entry_semantics
        .expect("smtp rcpt-denied should expose semantics");
    assert_eq!(rcpt_denied_semantics.category, "failure-path");
    assert_eq!(
        rcpt_denied_semantics.operator_focus,
        "recipient rejection during SMTP envelope construction"
    );
    assert_eq!(rcpt_denied_semantics.typical_signal.as_deref(), Some("550"));
    assert_eq!(
        rcpt_denied_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        rcpt_denied_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        rcpt_denied_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let data_denied =
        protocol_surface("smtp", "data-denied").expect("smtp data-denied should exist");
    let data_denied_semantics = data_denied
        .entry_semantics
        .expect("smtp data-denied should expose semantics");
    assert_eq!(
        data_denied_semantics.operator_focus,
        "message rejection after SMTP body handoff"
    );
    assert_eq!(data_denied_semantics.typical_signal.as_deref(), Some("550"));

    let kerberos_error =
        protocol_surface("kerberos", "as-error").expect("kerberos as-error should exist");
    let kerberos_error_semantics = kerberos_error
        .entry_semantics
        .expect("kerberos as-error should expose semantics");
    assert_eq!(kerberos_error_semantics.category, "failure-path");
    assert_eq!(
        kerberos_error_semantics.operator_focus,
        "initial Kerberos authentication exchange failed with explicit KRB-ERROR"
    );
    assert_eq!(
        kerberos_error_semantics.typical_signal.as_deref(),
        Some("KRB-ERROR")
    );
    assert_eq!(
        kerberos_error_semantics.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
    assert_eq!(
        kerberos_error_semantics.primary_failure_detail.as_deref(),
        Some("protocol_error")
    );
    assert_eq!(
        kerberos_error_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let smtp_mail = protocol_surface("smtp", "mail").expect("smtp mail should exist");
    let smtp_mail_semantics = smtp_mail
        .entry_semantics
        .expect("smtp mail should expose positive envelope sender semantics");
    assert_eq!(smtp_mail_semantics.category, "mail-envelope-sender-path");

    let kerberos_as = protocol_surface("kerberos", "as").expect("kerberos as should exist");
    let kerberos_as_semantics = kerberos_as
        .entry_semantics
        .expect("kerberos as should expose positive ticket semantics");
    assert_eq!(kerberos_as_semantics.category, "kerberos-as-path");
}

#[test]
fn ftp_and_ssh_login_denied_surfaces_expose_machine_readable_entry_semantics() {
    let ftp_denied = protocol_surface("ftp", "denied").expect("ftp denied should exist");
    let ftp_semantics = ftp_denied
        .entry_semantics
        .expect("ftp denied should expose semantics");
    assert_eq!(ftp_semantics.category, "failure-path");
    assert_eq!(
        ftp_semantics.operator_focus,
        "ftp login rejection after USER/PASS credential exchange"
    );
    assert_eq!(ftp_semantics.typical_signal.as_deref(), Some("530"));
    assert_eq!(
        ftp_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        ftp_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        ftp_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let ssh_denied = protocol_surface("ssh", "auth-denied").expect("ssh auth-denied should exist");
    let ssh_semantics = ssh_denied
        .entry_semantics
        .expect("ssh auth-denied should expose semantics");
    assert_eq!(ssh_semantics.category, "failure-path");
    assert_eq!(
        ssh_semantics.operator_focus,
        "ssh authentication rejection after explicit auth request"
    );
    assert!(ssh_semantics.typical_signal.is_none());
    assert_eq!(
        ssh_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        ssh_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        ssh_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let ftp_session = protocol_surface("ftp", "session").expect("ftp session should exist");
    let ftp_session_semantics = ftp_session
        .entry_semantics
        .expect("ftp session should expose semantics");
    assert_eq!(ftp_session_semantics.category, "ftp-session-path");

    let ssh_auth = protocol_surface("ssh", "auth").expect("ssh auth should exist");
    let ssh_auth_semantics = ssh_auth
        .entry_semantics
        .expect("ssh auth should expose semantics");
    assert_eq!(ssh_auth_semantics.category, "ssh-auth-path");
}

#[test]
fn amqp_auth_denied_surface_exposes_machine_readable_entry_semantics() {
    let amqp = protocol_surface("amqp", "auth-denied").expect("amqp auth-denied should exist");
    let semantics = amqp
        .entry_semantics
        .expect("amqp auth-denied should expose semantics");
    assert_eq!(semantics.category, "failure-path");
    assert_eq!(
        semantics.operator_focus,
        "broker connection close after AMQP start-ok credential or mechanism negotiation"
    );
    assert_eq!(
        semantics.typical_signal.as_deref(),
        Some("connection.close")
    );
    assert_eq!(
        semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let publish = protocol_surface("amqp", "publish").expect("amqp publish should exist");
    let publish_semantics = publish
        .entry_semantics
        .expect("amqp publish should expose positive publish semantics");
    assert_eq!(publish_semantics.category, "amqp-publish-path");
    assert_eq!(
        publish_semantics.typical_signal.as_deref(),
        Some("basic.publish + basic.ack")
    );

    let consume = protocol_surface("amqp", "consume").expect("amqp consume should exist");
    let consume_semantics = consume
        .entry_semantics
        .expect("amqp consume should expose positive consume semantics");
    assert_eq!(consume_semantics.category, "amqp-consume-path");
}
