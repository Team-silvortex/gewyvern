use std::collections::BTreeMap;

use leserpent_runtime::{EffectExecution, EffectExecutor, EffectLease};

mod gewyvern;

pub use gewyvern::{
    GEWYVERN_HEALTH_EFFECT_KIND, GEWYVERN_STATUS_REFRESH_EFFECT_KIND, GewyvernHealthAdapter,
    GewyvernStatusObservation, GewyvernStatusRefreshAdapter, GewyvernStatusRefreshRequest,
    GewyvernTarget,
};

pub trait EffectAdapter: Send {
    fn kind(&self) -> &str;
    fn execute(&mut self, payload: &[u8]) -> EffectExecution;
}

#[derive(Default)]
pub struct AdapterRegistry {
    adapters: BTreeMap<String, Box<dyn EffectAdapter>>,
}

impl AdapterRegistry {
    pub fn register(&mut self, adapter: impl EffectAdapter + 'static) -> Result<(), String> {
        let kind = adapter.kind().to_string();
        validate_id("adapter kind", &kind)?;
        if self.adapters.contains_key(&kind) {
            return Err(format!("adapter kind '{kind}' is already registered"));
        }
        self.adapters.insert(kind, Box::new(adapter));
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }
}

impl EffectExecutor for AdapterRegistry {
    fn execute(&mut self, lease: &EffectLease) -> EffectExecution {
        match self.adapters.get_mut(&lease.kind) {
            Some(adapter) => adapter.execute(&lease.payload),
            None => EffectExecution::Reject {
                error: format!("no adapter is registered for effect kind '{}'", lease.kind),
            },
        }
    }
}

fn validate_id(label: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(())
        .ok_or_else(|| format!("invalid {label}"))
}
