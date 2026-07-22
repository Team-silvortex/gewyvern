use super::*;

pub(super) fn native_memory_info(config: NativeLearningConfig) -> Result<String, String> {
    NativeLearningBackend::open(config)?.memory_info_json()
}

pub(super) fn native_memory_model_info(config: NativeLearningConfig) -> Result<String, String> {
    NativeLearningBackend::open(config)?.model_info_json()
}

pub(super) fn native_memory_versions(config: NativeLearningConfig) -> Result<String, String> {
    NativeLearningBackend::open(config)?.memory_versions_json()
}

pub(super) fn native_memory_snapshot(config: NativeLearningConfig) -> Result<String, String> {
    NativeLearningBackend::open(config)?.export_memory_json()
}

pub(super) fn native_protocol_capabilities(config: NativeLearningConfig) -> Result<String, String> {
    learning_backend_protocol_capabilities_json(&LearningBackendConfig::Native(config))
}

pub(super) fn clear_native_memory(config: NativeLearningConfig) -> Result<String, String> {
    NativeLearningBackend::open(config)?.clear_memory_json()
}

pub(super) fn import_native_memory(
    memory_snapshot_json: &str,
    strategy: &str,
    config: NativeLearningConfig,
) -> Result<String, String> {
    NativeLearningBackend::open(config)?
        .import_memory_with_strategy_json(memory_snapshot_json, strategy)
}

pub(super) fn plan_native_memory_transfer(
    memory_snapshot_json: &str,
    strategy: &str,
    config: NativeLearningConfig,
) -> Result<String, String> {
    learning_backend_memory_transfer_plan(
        memory_snapshot_json,
        strategy,
        &LearningBackendConfig::Native(config),
    )
}

pub(super) fn save_native_memory_slot(
    slot: &str,
    label: Option<&str>,
    note: Option<&str>,
    source: Option<&str>,
    config: NativeLearningConfig,
) -> Result<String, String> {
    NativeLearningBackend::open(config)?.save_memory_slot_json(slot, label, note, source)
}

pub(super) fn load_native_memory_slot(
    slot: &str,
    strategy: &str,
    config: NativeLearningConfig,
) -> Result<String, String> {
    NativeLearningBackend::open(config)?.load_memory_slot_json(slot, strategy)
}

pub(super) fn delete_native_memory_slot(
    slot: &str,
    config: NativeLearningConfig,
) -> Result<String, String> {
    NativeLearningBackend::open(config)?.delete_memory_slot_json(slot)
}

pub(super) fn parse_native_memory_strategy(
    args: &[String],
) -> Result<(String, NativeLearningConfig, usize), String> {
    let mut strategy = "replace".to_string();
    let mut index = 0;
    if let Some(flag) = args.first() {
        match flag.as_str() {
            "--merge" => {
                strategy = "merge".to_string();
                index = 1;
            }
            "--replace" => index = 1,
            _ => {}
        }
    }
    let (config, consumed) = parse_native_options(&args[index..])?;
    Ok((strategy, config, index + consumed))
}

pub(super) type NativeSlotMetadataOptions = (
    Option<String>,
    Option<String>,
    Option<String>,
    NativeLearningConfig,
    usize,
);

pub(super) fn parse_native_slot_metadata(
    args: &[String],
) -> Result<NativeSlotMetadataOptions, String> {
    let mut label = None;
    let mut note = None;
    let mut source = None;
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        let destination = match flag.as_str() {
            "--label" => &mut label,
            "--note" => &mut note,
            "--source" => &mut source,
            _ => break,
        };
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        *destination = Some(value.clone());
        index += 2;
    }
    let (config, consumed) = parse_native_options(&args[index..])?;
    Ok((label, note, source, config, index + consumed))
}

pub(super) fn python_memory_info(config: &PythonWorkerConfig) -> Result<String, String> {
    with_python_worker(config, |worker| worker.memory_info_json())
}

pub(super) fn python_memory_model_info(config: &PythonWorkerConfig) -> Result<String, String> {
    with_python_worker(config, |worker| worker.model_info_json())
}

pub(super) fn python_memory_versions(config: &PythonWorkerConfig) -> Result<String, String> {
    with_python_worker(config, |worker| worker.memory_versions_json())
}

pub(super) fn python_memory_snapshot(config: &PythonWorkerConfig) -> Result<String, String> {
    with_python_worker(config, |worker| worker.export_memory_json())
}

pub(super) fn protocol_capabilities(config: &PythonWorkerConfig) -> Result<String, String> {
    protocol_capabilities_json(config)
}

pub(super) fn clear_python_memory(config: &PythonWorkerConfig) -> Result<String, String> {
    with_python_worker(config, |worker| worker.clear_memory_json())
}

pub(super) fn import_python_memory(
    memory_snapshot_json: &str,
    strategy: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    with_python_worker(config, |worker| {
        worker.import_memory_with_strategy_json(memory_snapshot_json, strategy)
    })
}

pub(super) fn plan_python_memory_transfer(
    memory_snapshot_json: &str,
    strategy: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    python_memory_transfer_plan(memory_snapshot_json, strategy, config)
}

pub(super) fn save_python_memory_slot(
    slot: &str,
    label: Option<&str>,
    note: Option<&str>,
    source: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    with_python_worker(config, |worker| {
        worker.save_memory_slot_json(slot, label, note, source)
    })
}

pub(super) fn load_python_memory_slot(
    slot: &str,
    strategy: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    with_python_worker(config, |worker| {
        worker.load_memory_slot_json(slot, strategy)
    })
}

pub(super) fn delete_python_memory_slot(
    slot: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    with_python_worker(config, |worker| worker.delete_memory_slot_json(slot))
}

pub(super) fn parse_memory_strategy_and_python_options(
    args: &[String],
) -> Result<(String, PythonWorkerConfig, usize), String> {
    let mut strategy = "replace".to_string();
    let mut index = 0;
    if let Some(flag) = args.first() {
        match flag.as_str() {
            "--merge" => {
                strategy = "merge".to_string();
                index = 1;
            }
            "--replace" => {
                index = 1;
            }
            _ => {}
        }
    }
    let (config, consumed) = parse_python_worker_options(&args[index..])?;
    Ok((strategy, config, index + consumed))
}

pub(super) type SlotMetadataOptions = (
    Option<String>,
    Option<String>,
    Option<String>,
    PythonWorkerConfig,
    usize,
);

pub(super) fn parse_slot_metadata_and_python_options(
    args: &[String],
) -> Result<SlotMetadataOptions, String> {
    let mut label = None;
    let mut note = None;
    let mut source = None;
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--label" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --label".to_string())?;
                label = Some(value.clone());
                index += 2;
            }
            "--note" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --note".to_string())?;
                note = Some(value.clone());
                index += 2;
            }
            "--source" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --source".to_string())?;
                source = Some(value.clone());
                index += 2;
            }
            _ => break,
        }
    }
    let (config, consumed) = parse_python_worker_options(&args[index..])?;
    Ok((label, note, source, config, index + consumed))
}
