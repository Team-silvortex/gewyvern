pub mod advisory;
pub mod contract;
pub mod engine;
pub mod learning_backend;
pub mod native_learning;
pub mod python_worker;

pub use advisory::{MockMlAdvisoryEngine, MockRecommendationAugmenter, MockScoreRerankPass};
pub use contract::{AnalysisAugmentation, AnalysisSnapshotInput, EngineOutput, SnapshotParseError};
pub use engine::{
    CandidateAugmenter, ExternalAnalysisEngine, PassPipeline, RecommendationAugmenter, RerankPass,
    analyze_gewyvern_analysis_json, append_engine_output, engine_output_json,
};
pub use learning_backend::{
    LearningBackend, LearningBackendConfig, spawn_learning_backend, with_learning_backend,
};
pub use native_learning::{NativeLearningBackend, NativeLearningConfig};
pub use python_worker::{
    PythonWorkerClient, PythonWorkerConfig, default_python_worker_script, with_python_worker,
};
