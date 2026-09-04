use std::io;
use std::path::Path;

use gewyvern_install_contract::installer::{
    GEWYVERN_INSTALLER_SCHEMA_VERSION as DIRECT_INSTALLER_SCHEMA_VERSION,
    GewyvernInstallerRequest as DirectInstallerRequest,
};
use gewyvern_install_contract::retirement::{
    GEWYVERN_RETIREMENT_SCHEMA_VERSION as DIRECT_RETIREMENT_SCHEMA_VERSION,
    GewyvernRetirementRequest as DirectRetirementRequest,
};
use leserpent_protocol::gewyvern_installer::{
    GEWYVERN_INSTALLER_SCHEMA_VERSION as COMPAT_INSTALLER_SCHEMA_VERSION,
    GewyvernInstallerRequest as CompatInstallerRequest, decode_gewyvern_installer_request,
};
use leserpent_protocol::gewyvern_retirement::{
    GEWYVERN_RETIREMENT_SCHEMA_VERSION as COMPAT_RETIREMENT_SCHEMA_VERSION,
    GewyvernRetirementRequest as CompatRetirementRequest,
};
use silvortex_bounded_io::BoundedFile;

fn accepts_direct_installer(_: DirectInstallerRequest) {}

fn accepts_direct_retirement(_: DirectRetirementRequest) {}

#[test]
fn old_installer_and_retirement_paths_preserve_type_identity() {
    assert_eq!(
        COMPAT_INSTALLER_SCHEMA_VERSION,
        DIRECT_INSTALLER_SCHEMA_VERSION
    );
    assert_eq!(
        COMPAT_RETIREMENT_SCHEMA_VERSION,
        DIRECT_RETIREMENT_SCHEMA_VERSION
    );

    let request: CompatInstallerRequest = decode_gewyvern_installer_request(include_bytes!(
        "../../gewyvern-install-contract/tests/fixtures/gewyvern-installer-request-v1.json"
    ))
    .unwrap();
    accepts_direct_installer(request);

    let retirement: Option<CompatRetirementRequest> = None;
    if let Some(retirement) = retirement {
        accepts_direct_retirement(retirement);
    }
}

#[test]
fn old_transport_safety_path_preserves_function_and_return_types() {
    let open: fn(&Path, u64) -> io::Result<BoundedFile> =
        leserpent_protocol::transport_safety::open_bounded_regular_file;
    let _ = open;
    assert_eq!(
        leserpent_protocol::transport_safety::MAX_RESOLVED_ADDRESSES,
        silvortex_bounded_io::MAX_RESOLVED_ADDRESSES
    );
}
