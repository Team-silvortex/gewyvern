use std::fs;
use std::path::Path;

use crate::certificate_inventory::CertificateAssetKind;
use x509_parser::pem::Pem;
use x509_parser::prelude::{FromDer, X509Certificate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateValidityWindow {
    pub certificate_count: usize,
    pub earliest_not_before_unix_ms: Option<i128>,
    pub earliest_not_after_unix_ms: Option<i128>,
    pub latest_not_after_unix_ms: Option<i128>,
}

pub fn inspect_certificate_validity(
    path: &Path,
    kind: CertificateAssetKind,
) -> Option<CertificateValidityWindow> {
    if !matches!(
        kind,
        CertificateAssetKind::CertificatePem
            | CertificateAssetKind::ChainPem
            | CertificateAssetKind::BundlePem
            | CertificateAssetKind::UnknownPem
    ) {
        return None;
    }
    let Ok(contents) = fs::read(path) else {
        return None;
    };
    certificate_validity_from_pem_bytes(&contents)
}

pub fn certificate_validity_from_pem_bytes(contents: &[u8]) -> Option<CertificateValidityWindow> {
    let mut certificate_count = 0usize;
    let mut earliest_not_before_unix_ms: Option<i128> = None;
    let mut earliest_not_after_unix_ms: Option<i128> = None;
    let mut latest_not_after_unix_ms: Option<i128> = None;

    for pem in Pem::iter_from_buffer(contents) {
        let Ok(pem) = pem else {
            continue;
        };
        let Ok((_, certificate)) = X509Certificate::from_der(&pem.contents) else {
            continue;
        };
        certificate_count += 1;
        let not_before = i128::from(certificate.validity().not_before.timestamp()) * 1000;
        let not_after = i128::from(certificate.validity().not_after.timestamp()) * 1000;
        earliest_not_before_unix_ms = Some(match earliest_not_before_unix_ms {
            Some(current) => current.min(not_before),
            None => not_before,
        });
        earliest_not_after_unix_ms = Some(match earliest_not_after_unix_ms {
            Some(current) => current.min(not_after),
            None => not_after,
        });
        latest_not_after_unix_ms = Some(match latest_not_after_unix_ms {
            Some(current) => current.max(not_after),
            None => not_after,
        });
    }

    (certificate_count > 0).then_some(CertificateValidityWindow {
        certificate_count,
        earliest_not_before_unix_ms,
        earliest_not_after_unix_ms,
        latest_not_after_unix_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIIDETCCAfmgAwIBAgIUZCW1Sx3Ms0VIC3Qka3U+uyMAiqAwDQYJKoZIhvcNAQEL\n\
BQAwGDEWMBQGA1UEAwwNZ2V3eXZlcm4tdGVzdDAeFw0yNjA2MjEwNTM0MjZaFw0z\n\
NjA2MTgwNTM0MjZaMBgxFjAUBgNVBAMMDWdld3l2ZXJuLXRlc3QwggEiMA0GCSqG\n\
SIb3DQEBAQUAA4IBDwAwggEKAoIBAQC7FxBX6XyTZ5v0t2u3aeVE89wUmT+WMyUt\n\
J3n/LIuS72S++4u/NzyIPXhWFlPe0XSVqx9Je4YgmLLLfyxlYy5c5taiyUNUc+RI\n\
Z2j3ahIVTVKgyyn+UOKhDPREqcFb/n/RF4QfuzWTqDtvKRSQLjRmG1H1gekoSW7F\n\
DcDmPy/dSSq//MK64HUd2MrGzNXF2a+ZCZBX5eQDg+FV6lvth7wNvML0/vhB2614\n\
Fphm0Jx77+4sQoCQZpcmW6hi75OwxakARXkFiEhJ+ZPk9drhvL7Rv+cVGekdpftd\n\
VMFbGB6pnJMoAr6SsFrWZ1oBF0fQq9xoDuvjZHJRe79pwMe6nV1hAgMBAAGjUzBR\n\
MB0GA1UdDgQWBBSYL1qeHSL4dTx+6sUVfUYuZexYbzAfBgNVHSMEGDAWgBSYL1qe\n\
HSL4dTx+6sUVfUYuZexYbzAPBgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3DQEBCwUA\n\
A4IBAQBNDri7E69Eg/WFhEqljKF1IwrWBVY+gsemicvBA5iEvMczduXuc1eZiTLd\n\
oLNZs8OONx478RpgK5J0OEHPW1l8UMRmsO2CCU2BpywbLB1rysZ3td2v0Tl5wPwj\n\
CTvjOyw3FFg0UNwLkg1zy/GCR/GupWRCatAcGR/kYDyqV8vkTndn1+Je7H7fxbX8\n\
R9zQ2WloXiBFyOc17gpeIIioPTOkxPKeyLMs9rmmCrV9unrhNG6BeipJZzYVnyap\n\
7HdvUZvuyNni3YouorNinME7CewXYswxkHygInpp6Ni9QuKvoO5uFrcZK4cQO7XT\n\
m1mukAGYQJ1NnRIr044e4WtIhEi7\n\
-----END CERTIFICATE-----\n";

    #[test]
    fn pem_validity_extracts_certificate_window() {
        let validity = certificate_validity_from_pem_bytes(TEST_CERT_PEM.as_bytes()).unwrap();
        assert_eq!(validity.certificate_count, 1);
        assert_eq!(
            validity.earliest_not_before_unix_ms,
            Some(1_782_020_066_000)
        );
        assert_eq!(validity.earliest_not_after_unix_ms, Some(2_097_380_066_000));
        assert_eq!(validity.latest_not_after_unix_ms, Some(2_097_380_066_000));
    }
}
