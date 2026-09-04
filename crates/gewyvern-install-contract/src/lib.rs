//! Narrow cross-plane contracts used by Gewyvern installers and controllers.

pub use silvortex_identity::{
    CredentialHandle, IdentityError, ProvisioningId, RetirementId, RuntimeId,
};

pub mod installer;
pub mod retirement;
