use super::*;

#[test]
fn protocol_entry_surface_endpoint_exposes_mail_auth_denied_semantics() {
    let snapshot = ApiSnapshot::default();

    let (imap_status, _, imap_body) =
        api_response_for_request("/v1/protocols/imap/entries/auth-denied/surface.json", &snapshot);
    assert_eq!(imap_status, 200);
    assert!(imap_body.contains("\"protocol\":\"imap\""));
    assert!(imap_body.contains("\"entry\":\"auth-denied\""));
    assert!(imap_body.contains("\"entry_semantics\":{"));
    assert!(imap_body.contains("\"category\":\"failure-path\""));
    assert!(
        imap_body.contains(
            "\"operator_focus\":\"mailbox login rejection after LOGIN credential exchange\""
        )
    );
    assert!(imap_body.contains("\"typical_signal\":\"NO\""));
    assert!(imap_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(imap_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(imap_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));

    let (pop3_status, _, pop3_body) =
        api_response_for_request("/v1/protocols/pop3/entries/auth-denied/surface.json", &snapshot);
    assert_eq!(pop3_status, 200);
    assert!(pop3_body.contains("\"protocol\":\"pop3\""));
    assert!(pop3_body.contains("\"entry\":\"auth-denied\""));
    assert!(pop3_body.contains("\"entry_semantics\":{"));
    assert!(pop3_body.contains("\"category\":\"failure-path\""));
    assert!(
        pop3_body.contains(
            "\"operator_focus\":\"mailbox password rejection after USER/PASS credential exchange\""
        )
    );
    assert!(pop3_body.contains("\"typical_signal\":\"-ERR\""));
    assert!(pop3_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(pop3_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(pop3_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_management_denied_semantics() {
    let snapshot = ApiSnapshot::default();

    let (snmp_status, _, snmp_body) = api_response_for_request(
        "/v1/protocols/snmp/entries/unauthorized/surface.json",
        &snapshot,
    );
    assert_eq!(snmp_status, 200);
    assert!(snmp_body.contains("\"protocol\":\"snmp\""));
    assert!(snmp_body.contains("\"entry\":\"unauthorized\""));
    assert!(snmp_body.contains("\"entry_semantics\":{"));
    assert!(snmp_body.contains("\"category\":\"failure-path\""));
    assert!(snmp_body.contains(
        "\"operator_focus\":\"authorization failure report after SNMPv3 access evaluation\""
    ));
    assert!(snmp_body.contains("\"typical_signal\":null"));
    assert!(snmp_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(snmp_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(snmp_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));

    let (stun_status, _, stun_body) = api_response_for_request(
        "/v1/protocols/stun/entries/binding-error/surface.json",
        &snapshot,
    );
    assert_eq!(stun_status, 200);
    assert!(stun_body.contains("\"protocol\":\"stun\""));
    assert!(stun_body.contains("\"entry\":\"binding-error\""));
    assert!(stun_body.contains("\"entry_semantics\":{"));
    assert!(stun_body.contains("\"category\":\"failure-path\""));
    assert!(stun_body.contains(
        "\"operator_focus\":\"explicit binding failure response instead of successful reachability confirmation\""
    ));
    assert!(stun_body.contains("\"typical_signal\":null"));
    assert!(stun_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(stun_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(stun_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_smtp_and_kerberos_failure_semantics() {
    let snapshot = ApiSnapshot::default();

    let (smtp_auth_status, _, smtp_auth_body) = api_response_for_request(
        "/v1/protocols/smtp/entries/auth-denied/surface.json",
        &snapshot,
    );
    assert_eq!(smtp_auth_status, 200);
    assert!(smtp_auth_body.contains("\"protocol\":\"smtp\""));
    assert!(smtp_auth_body.contains("\"entry\":\"auth-denied\""));
    assert!(smtp_auth_body.contains("\"entry_semantics\":{"));
    assert!(smtp_auth_body.contains("\"category\":\"failure-path\""));
    assert!(
        smtp_auth_body
            .contains("\"operator_focus\":\"smtp authentication rejection after explicit AUTH exchange\"")
    );
    assert!(smtp_auth_body.contains("\"typical_signal\":\"535\""));
    assert!(smtp_auth_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(smtp_auth_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(smtp_auth_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));

    let (smtp_status, _, smtp_body) = api_response_for_request(
        "/v1/protocols/smtp/entries/rcpt-denied/surface.json",
        &snapshot,
    );
    assert_eq!(smtp_status, 200);
    assert!(smtp_body.contains("\"protocol\":\"smtp\""));
    assert!(smtp_body.contains("\"entry\":\"rcpt-denied\""));
    assert!(smtp_body.contains("\"entry_semantics\":{"));
    assert!(smtp_body.contains("\"category\":\"failure-path\""));
    assert!(
        smtp_body.contains(
            "\"operator_focus\":\"recipient rejection during SMTP envelope construction\""
        )
    );
    assert!(smtp_body.contains("\"typical_signal\":\"550\""));
    assert!(smtp_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(smtp_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(smtp_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));

    let (kerberos_status, _, kerberos_body) = api_response_for_request(
        "/v1/protocols/kerberos/entries/as-error/surface.json",
        &snapshot,
    );
    assert_eq!(kerberos_status, 200);
    assert!(kerberos_body.contains("\"protocol\":\"kerberos\""));
    assert!(kerberos_body.contains("\"entry\":\"as-error\""));
    assert!(kerberos_body.contains("\"entry_semantics\":{"));
    assert!(kerberos_body.contains("\"category\":\"failure-path\""));
    assert!(kerberos_body.contains(
        "\"operator_focus\":\"initial Kerberos authentication exchange failed with explicit KRB-ERROR\""
    ));
    assert!(kerberos_body.contains("\"typical_signal\":\"KRB-ERROR\""));
    assert!(kerberos_body.contains("\"primary_failure_mode\":\"semantic_error\""));
    assert!(kerberos_body.contains("\"primary_failure_detail\":\"protocol_error\""));
    assert!(kerberos_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn protocol_entry_surface_endpoint_exposes_ftp_and_ssh_login_denied_semantics() {
    let snapshot = ApiSnapshot::default();

    let (ftp_status, _, ftp_body) =
        api_response_for_request("/v1/protocols/ftp/entries/denied/surface.json", &snapshot);
    assert_eq!(ftp_status, 200);
    assert!(ftp_body.contains("\"protocol\":\"ftp\""));
    assert!(ftp_body.contains("\"entry\":\"denied\""));
    assert!(ftp_body.contains("\"entry_semantics\":{"));
    assert!(ftp_body.contains("\"category\":\"failure-path\""));
    assert!(
        ftp_body.contains("\"operator_focus\":\"ftp login rejection after USER/PASS credential exchange\"")
    );
    assert!(ftp_body.contains("\"typical_signal\":\"530\""));
    assert!(ftp_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(ftp_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(ftp_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));

    let (ssh_status, _, ssh_body) = api_response_for_request(
        "/v1/protocols/ssh/entries/auth-denied/surface.json",
        &snapshot,
    );
    assert_eq!(ssh_status, 200);
    assert!(ssh_body.contains("\"protocol\":\"ssh\""));
    assert!(ssh_body.contains("\"entry\":\"auth-denied\""));
    assert!(ssh_body.contains("\"entry_semantics\":{"));
    assert!(ssh_body.contains("\"category\":\"failure-path\""));
    assert!(
        ssh_body.contains(
            "\"operator_focus\":\"ssh authentication rejection after explicit auth request\""
        )
    );
    assert!(ssh_body.contains("\"typical_signal\":null"));
    assert!(ssh_body.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(ssh_body.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(ssh_body.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}
