use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const MSSQL_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mssql",
    default_entry: "query",
    entries: &[
        ProtocolEntryProfile {
            mode: "prelogin",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_prelogin_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "login",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_login_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "response",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_response_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "colmetadata",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_colmetadata_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "row",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_row_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "done",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_done_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "envchange",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_envchange_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mssql_error_path.gewy",
        },
    ],
};
