use std::env;
use std::fs;
use std::path::PathBuf;

use gewyvern::validation_harness::{
    ValidationError, run_external_engine_roundtrip_demo, run_linux_attach_smoke,
    run_linux_kprobe_smoke, run_linux_tc_smoke, run_socket_roundtrip_demo,
    run_training_dataset_roundtrip_demo,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), ValidationError> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        print_help();
        return Ok(());
    }

    let command = args.remove(0);

    match command.as_str() {
        "socket-roundtrip" => {
            let socket_target = args.get(0).cloned().unwrap_or_else(default_socket_path);
            let template = args.get(1).cloned().unwrap_or_else(|| "udp".into());
            let output_path = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(default_socket_output_path);
            let socket_kind = args.get(3).cloned().unwrap_or_else(|| "unix".into());

            let _report = run_socket_roundtrip_demo(
                Some(&socket_target),
                Some(&template),
                Some(output_path.clone()),
                Some(&socket_kind),
            )?;
            println!("{}", fs::read_to_string(output_path)?);
            Ok(())
        }
        "external-engine-roundtrip" => {
            let ingest_addr = args.get(0).cloned();
            let api_addr = args.get(1).cloned();
            let template = args.get(2).cloned();
            let analysis_out = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(default_external_engine_analysis_path);
            let engine_out = args
                .get(4)
                .map(PathBuf::from)
                .unwrap_or_else(default_external_engine_output_path);
            let target_path_segment = args.get(5).map(|value| value.as_str());

            let _report = run_external_engine_roundtrip_demo(
                ingest_addr.as_deref(),
                api_addr.as_deref(),
                template.as_deref(),
                Some(analysis_out.clone()),
                Some(engine_out.clone()),
                target_path_segment,
                None,
                None,
            )?;

            println!("analysis_json={}", analysis_out.display());
            println!("external_engine_output={}", engine_out.display());
            if engine_out.exists() && fs::metadata(&engine_out)?.len() > 0 {
                println!("{}", fs::read_to_string(engine_out)?);
            }
            Ok(())
        }
        "training-roundtrip" => {
            let api_addr = args
                .get(0)
                .cloned()
                .unwrap_or_else(|| "127.0.0.1:9910".into());
            let out_dir = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_training_out_dir);
            let target_path_segment = args.get(2).map(|value| value.as_str());
            let limit = args
                .get(3)
                .map(|value| value.parse::<usize>())
                .transpose()
                .map_err(|_| ValidationError::new("limit must be an integer"))?;

            let _report = run_training_dataset_roundtrip_demo(
                Some(&api_addr),
                Some(out_dir.clone()),
                target_path_segment,
                limit,
            )?;
            let summary_path = out_dir.join("roundtrip-summary.json");
            println!("{}", fs::read_to_string(summary_path)?);
            Ok(())
        }
        "linux-attach-smoke" => {
            let hookpoint = args
                .get(0)
                .cloned()
                .unwrap_or_else(|| "syscalls/sys_enter_nanosleep".into());
            let _report = run_linux_attach_smoke(&hookpoint, None)?;
            Ok(())
        }
        "linux-kprobe-smoke" => {
            let symbol = args
                .get(0)
                .cloned()
                .unwrap_or_else(|| "ip_route_output_flow".into());
            let _report = run_linux_kprobe_smoke(&symbol, None)?;
            Ok(())
        }
        "linux-tc-smoke" => {
            let dev = args.get(0).cloned().unwrap_or_else(|| "eth0".into());
            let _report = run_linux_tc_smoke(&dev, None)?;
            Ok(())
        }
        _ => {
            print_help();
            Err(ValidationError::new("unknown command"))
        }
    }
}

fn default_socket_path() -> String {
    env::var("GEWY_DEMO_SOCKET_PATH")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or_else(|| {
            env::temp_dir()
                .join(format!("gewyvern-demo-{}.sock", std::process::id()))
                .to_string_lossy()
                .into_owned()
        })
}

fn default_socket_output_path() -> PathBuf {
    env::var("GEWY_DEMO_OUTPUT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("gewyvern-demo-output.json"))
}

fn default_external_engine_analysis_path() -> PathBuf {
    env::var("GEWY_EXTERNAL_ANALYSIS_JSON")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("gewyvern-analysis.json"))
}

fn default_external_engine_output_path() -> PathBuf {
    env::var("GEWY_EXTERNAL_ENGINE_JSON")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("external-engine-output.json"))
}

fn default_training_out_dir() -> PathBuf {
    env::var("GEWY_TRAINING_DEMO_OUT_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("gewyvern-training-dataset-demo"))
}

fn print_help() {
    println!("Usage:");
    println!(
        "  gewyvern_native_call socket-roundtrip [socket-target] [template] [out-json] [unix|tcp]"
    );
    println!(
        "  gewyvern_native_call external-engine-roundtrip [ingest-addr] [api-addr] [template] [analysis-json] [engine-json] [target-segment]"
    );
    println!(
        "  gewyvern_native_call training-roundtrip [api-addr] [out-dir] [target-segment] [limit]"
    );
    println!("  gewyvern_native_call linux-attach-smoke [hookpoint]");
    println!("  gewyvern_native_call linux-kprobe-smoke [symbol]");
    println!("  gewyvern_native_call linux-tc-smoke [device]");
}
