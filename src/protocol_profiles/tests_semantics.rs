use super::protocol_surface;

#[test]
fn redis_failure_surfaces_expose_machine_readable_entry_semantics() {
    let moved = protocol_surface("redis", "moved").expect("redis moved surface should exist");
    let moved_semantics = moved
        .entry_semantics
        .expect("redis moved should expose semantics");
    assert_eq!(moved_semantics.category, "failure-path");
    assert_eq!(
        moved_semantics.operator_focus,
        "cluster slot redirect that requires target remap"
    );
    assert_eq!(moved_semantics.typical_signal.as_deref(), Some("-MOVED"));
    assert_eq!(
        moved_semantics.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
    assert_eq!(
        moved_semantics.primary_failure_detail.as_deref(),
        Some("protocol_error")
    );
    assert_eq!(
        moved_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let readonly =
        protocol_surface("redis", "readonly").expect("redis readonly surface should exist");
    let readonly_semantics = readonly
        .entry_semantics
        .expect("redis readonly should expose semantics");
    assert_eq!(
        readonly_semantics.operator_focus,
        "replica write refusal or readonly placement mismatch"
    );
    assert_eq!(
        readonly_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        readonly_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );

    let zadd = protocol_surface("redis", "zadd").expect("redis zadd surface should exist");
    assert!(zadd.entry_semantics.is_none());
}

#[test]
fn socks5_denial_surfaces_expose_machine_readable_entry_semantics() {
    let denied = protocol_surface("socks5", "denied").expect("socks5 denied surface should exist");
    let denied_semantics = denied
        .entry_semantics
        .expect("socks5 denied should expose semantics");
    assert_eq!(denied_semantics.category, "failure-path");
    assert_eq!(
        denied_semantics.operator_focus,
        "upstream connect refusal after no-auth method selection"
    );
    assert!(denied_semantics.typical_signal.is_none());
    assert_eq!(
        denied_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        denied_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        denied_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let auth_denied =
        protocol_surface("socks5", "auth-denied").expect("socks5 auth-denied should exist");
    let auth_denied_semantics = auth_denied
        .entry_semantics
        .expect("socks5 auth-denied should expose semantics");
    assert_eq!(
        auth_denied_semantics.operator_focus,
        "username/password rejection during proxy auth exchange"
    );

    let auth_connect_denied = protocol_surface("socks5", "auth-connect-denied")
        .expect("socks5 auth-connect-denied should exist");
    let auth_connect_denied_semantics = auth_connect_denied
        .entry_semantics
        .expect("socks5 auth-connect-denied should expose semantics");
    assert_eq!(
        auth_connect_denied_semantics.operator_focus,
        "upstream connect refusal after authenticated proxy setup"
    );

    let auth = protocol_surface("socks5", "auth").expect("socks5 auth surface should exist");
    assert!(auth.entry_semantics.is_none());
}

#[test]
fn http_and_ldap_denial_surfaces_expose_machine_readable_entry_semantics() {
    let http_denied = protocol_surface("http", "denied").expect("http denied surface should exist");
    let http_semantics = http_denied
        .entry_semantics
        .expect("http denied should expose semantics");
    assert_eq!(http_semantics.category, "failure-path");
    assert_eq!(
        http_semantics.operator_focus,
        "proxy tunnel refusal after CONNECT policy evaluation"
    );
    assert_eq!(http_semantics.typical_signal.as_deref(), Some("403"));
    assert_eq!(
        http_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        http_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        http_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let ldap_denied =
        protocol_surface("ldap", "bind-denied").expect("ldap bind-denied surface should exist");
    let ldap_semantics = ldap_denied
        .entry_semantics
        .expect("ldap bind-denied should expose semantics");
    assert_eq!(ldap_semantics.category, "failure-path");
    assert_eq!(
        ldap_semantics.operator_focus,
        "directory credential or bind-policy rejection during auth establishment"
    );
    assert!(ldap_semantics.typical_signal.is_none());
    assert_eq!(
        ldap_semantics.primary_failure_mode.as_deref(),
        Some("server_denied")
    );
    assert_eq!(
        ldap_semantics.primary_failure_detail.as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        ldap_semantics.primary_failure_basis.as_deref(),
        Some("direct_protocol_signal")
    );

    let http_connect = protocol_surface("http", "connect").expect("http connect should exist");
    assert!(http_connect.entry_semantics.is_none());

    let ldap_bind = protocol_surface("ldap", "bind").expect("ldap bind should exist");
    assert!(ldap_bind.entry_semantics.is_none());
}

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
    assert!(imap_auth.entry_semantics.is_none());

    let pop3_auth = protocol_surface("pop3", "auth").expect("pop3 auth should exist");
    assert!(pop3_auth.entry_semantics.is_none());
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

    let snmp_report = protocol_surface("snmp", "report").expect("snmp report should exist");
    assert!(snmp_report.entry_semantics.is_none());

    let stun_binding = protocol_surface("stun", "binding").expect("stun binding should exist");
    assert!(stun_binding.entry_semantics.is_none());
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
    assert!(smtp_mail.entry_semantics.is_none());

    let kerberos_as = protocol_surface("kerberos", "as").expect("kerberos as should exist");
    assert!(kerberos_as.entry_semantics.is_none());
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
    assert!(ftp_session.entry_semantics.is_none());

    let ssh_auth = protocol_surface("ssh", "auth").expect("ssh auth should exist");
    assert!(ssh_auth.entry_semantics.is_none());
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
}
