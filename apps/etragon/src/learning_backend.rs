use crate::{NativeLearningBackend, NativeLearningConfig, PythonWorkerClient, PythonWorkerConfig};

pub trait LearningBackend: Send {
    fn analyze_json(&mut self, snapshot_json: &str) -> Result<String, String>;
    fn train_json_with_weight(
        &mut self,
        snapshot_json: &str,
        label: &str,
        weight: f64,
    ) -> Result<String, String>;
    fn memory_info_json(&mut self) -> Result<String, String>;
    fn model_info_json(&mut self) -> Result<String, String>;
    fn memory_versions_json(&mut self) -> Result<String, String>;
    fn export_memory_json(&mut self) -> Result<String, String>;
    fn import_memory_json(&mut self, memory_snapshot_json: &str) -> Result<String, String>;
    fn import_memory_with_strategy_json(
        &mut self,
        memory_snapshot_json: &str,
        strategy: &str,
    ) -> Result<String, String>;
    fn save_memory_slot_json(
        &mut self,
        slot: &str,
        label: Option<&str>,
        note: Option<&str>,
        source: Option<&str>,
    ) -> Result<String, String>;
    fn load_memory_slot_json(&mut self, slot: &str, strategy: &str) -> Result<String, String>;
    fn delete_memory_slot_json(&mut self, slot: &str) -> Result<String, String>;
    fn clear_memory_json(&mut self) -> Result<String, String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LearningBackendConfig {
    Native(NativeLearningConfig),
    Python(PythonWorkerConfig),
}

impl LearningBackendConfig {
    pub fn native(state_file: Option<std::path::PathBuf>) -> Self {
        Self::Native(NativeLearningConfig { state_file })
    }
}

pub fn spawn_learning_backend(
    config: &LearningBackendConfig,
) -> Result<Box<dyn LearningBackend>, String> {
    match config {
        LearningBackendConfig::Native(config) => {
            Ok(Box::new(NativeLearningBackend::open(config.clone())?))
        }
        LearningBackendConfig::Python(config) => Ok(Box::new(PythonWorkerClient::spawn(config)?)),
    }
}

pub fn with_learning_backend<T, F>(config: &LearningBackendConfig, f: F) -> Result<T, String>
where
    F: FnOnce(&mut dyn LearningBackend) -> Result<T, String>,
{
    let mut backend = spawn_learning_backend(config)?;
    f(backend.as_mut())
}

impl LearningBackend for NativeLearningBackend {
    fn analyze_json(&mut self, snapshot_json: &str) -> Result<String, String> {
        NativeLearningBackend::analyze_json(self, snapshot_json)
    }

    fn train_json_with_weight(
        &mut self,
        snapshot_json: &str,
        label: &str,
        weight: f64,
    ) -> Result<String, String> {
        NativeLearningBackend::train_json_with_weight(self, snapshot_json, label, weight)
    }

    fn memory_info_json(&mut self) -> Result<String, String> {
        NativeLearningBackend::memory_info_json(self)
    }

    fn model_info_json(&mut self) -> Result<String, String> {
        NativeLearningBackend::model_info_json(self)
    }

    fn memory_versions_json(&mut self) -> Result<String, String> {
        NativeLearningBackend::memory_versions_json(self)
    }

    fn export_memory_json(&mut self) -> Result<String, String> {
        NativeLearningBackend::export_memory_json(self)
    }

    fn import_memory_json(&mut self, memory_snapshot_json: &str) -> Result<String, String> {
        NativeLearningBackend::import_memory_json(self, memory_snapshot_json)
    }

    fn import_memory_with_strategy_json(
        &mut self,
        memory_snapshot_json: &str,
        strategy: &str,
    ) -> Result<String, String> {
        NativeLearningBackend::import_memory_with_strategy_json(
            self,
            memory_snapshot_json,
            strategy,
        )
    }

    fn save_memory_slot_json(
        &mut self,
        slot: &str,
        label: Option<&str>,
        note: Option<&str>,
        source: Option<&str>,
    ) -> Result<String, String> {
        NativeLearningBackend::save_memory_slot_json(self, slot, label, note, source)
    }

    fn load_memory_slot_json(&mut self, slot: &str, strategy: &str) -> Result<String, String> {
        NativeLearningBackend::load_memory_slot_json(self, slot, strategy)
    }

    fn delete_memory_slot_json(&mut self, slot: &str) -> Result<String, String> {
        NativeLearningBackend::delete_memory_slot_json(self, slot)
    }

    fn clear_memory_json(&mut self) -> Result<String, String> {
        NativeLearningBackend::clear_memory_json(self)
    }
}

impl LearningBackend for PythonWorkerClient {
    fn analyze_json(&mut self, snapshot_json: &str) -> Result<String, String> {
        PythonWorkerClient::analyze_json(self, snapshot_json)
    }

    fn train_json_with_weight(
        &mut self,
        snapshot_json: &str,
        label: &str,
        weight: f64,
    ) -> Result<String, String> {
        PythonWorkerClient::train_json_with_weight(self, snapshot_json, label, weight)
    }

    fn memory_info_json(&mut self) -> Result<String, String> {
        PythonWorkerClient::memory_info_json(self)
    }

    fn model_info_json(&mut self) -> Result<String, String> {
        PythonWorkerClient::model_info_json(self)
    }

    fn memory_versions_json(&mut self) -> Result<String, String> {
        PythonWorkerClient::memory_versions_json(self)
    }

    fn export_memory_json(&mut self) -> Result<String, String> {
        PythonWorkerClient::export_memory_json(self)
    }

    fn import_memory_json(&mut self, memory_snapshot_json: &str) -> Result<String, String> {
        PythonWorkerClient::import_memory_json(self, memory_snapshot_json)
    }

    fn import_memory_with_strategy_json(
        &mut self,
        memory_snapshot_json: &str,
        strategy: &str,
    ) -> Result<String, String> {
        PythonWorkerClient::import_memory_with_strategy_json(self, memory_snapshot_json, strategy)
    }

    fn save_memory_slot_json(
        &mut self,
        slot: &str,
        label: Option<&str>,
        note: Option<&str>,
        source: Option<&str>,
    ) -> Result<String, String> {
        PythonWorkerClient::save_memory_slot_json(self, slot, label, note, source)
    }

    fn load_memory_slot_json(&mut self, slot: &str, strategy: &str) -> Result<String, String> {
        PythonWorkerClient::load_memory_slot_json(self, slot, strategy)
    }

    fn delete_memory_slot_json(&mut self, slot: &str) -> Result<String, String> {
        PythonWorkerClient::delete_memory_slot_json(self, slot)
    }

    fn clear_memory_json(&mut self) -> Result<String, String> {
        PythonWorkerClient::clear_memory_json(self)
    }
}
