use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use super::command::{
    DOTNET_PROOF_TIMEOUT, ValidationError, repo_root, run_command_output_with_timeout,
};

const CRASH_HARNESS_ASSEMBLY_ENV: &str = "LESERPENT_TEST_CRASH_HARNESS_ASSEMBLY";

pub(crate) fn run_locked_dotnet_test(
    project: &str,
    filter: Option<&str>,
    artifacts_path: &Path,
    log_path: &Path,
) -> Result<usize, ValidationError> {
    if artifacts_path.exists() {
        fs::remove_dir_all(artifacts_path)?;
    }
    let mut command = Command::new("dotnet");
    command
        .current_dir(repo_root())
        .env("DOTNET_CLI_UI_LANGUAGE", "en-US")
        .env("VSLANG", "1033")
        .env(
            CRASH_HARNESS_ASSEMBLY_ENV,
            artifacts_path
                .join("bin")
                .join("Leserpent.RuntimeDeletionCrashHarness")
                .join("release")
                .join("Leserpent.RuntimeDeletionCrashHarness.dll"),
        )
        .args([
            "test",
            project,
            "--configuration",
            "Release",
            "--artifacts-path",
        ])
        .arg(artifacts_path)
        .args([
            "-p:RestoreLockedMode=true",
            "--logger",
            "console;verbosity=minimal",
        ]);
    if let Some(filter) = filter {
        command.args(["--filter", filter]);
    }

    let output = run_command_output_with_timeout(
        &mut command,
        DOTNET_PROOF_TIMEOUT,
        &format!("locked dotnet tests for {project}"),
    )?;
    write_output(log_path, &output)?;
    let result = if output.status.success() {
        dotnet_passed_test_count(&output.stdout, &output.stderr)
    } else {
        Err(ValidationError::new(format!(
            "dotnet test failed with status {}",
            output.status
        )))
    };
    let cleanup = if artifacts_path.exists() {
        fs::remove_dir_all(artifacts_path)
    } else {
        Ok(())
    };
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(ValidationError::new(format!(
            "failed to clean dotnet test artifacts: {error}"
        ))),
        (Ok(count), Ok(())) => Ok(count),
    }
}

fn write_output(path: &Path, output: &Output) -> Result<(), ValidationError> {
    let mut transcript = Vec::new();
    transcript.extend_from_slice(b"stdout:\n");
    transcript.extend_from_slice(&output.stdout);
    transcript.extend_from_slice(b"\n\nstderr:\n");
    transcript.extend_from_slice(&output.stderr);
    fs::write(path, transcript)?;
    Ok(())
}

fn dotnet_passed_test_count(stdout: &[u8], stderr: &[u8]) -> Result<usize, ValidationError> {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let summaries = stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| line.trim_start().starts_with("Passed!"))
        .collect::<Vec<_>>();
    if summaries.len() != 1 {
        return Err(ValidationError::new(format!(
            "dotnet test output contained {} success summaries, expected exactly one",
            summaries.len()
        )));
    }
    let summary = summaries[0];
    let failed = labeled_dotnet_count(summary, "Failed:")?;
    let passed = labeled_dotnet_count(summary, "Passed:")?;
    let skipped = labeled_dotnet_count(summary, "Skipped:")?;
    let total = labeled_dotnet_count(summary, "Total:")?;
    if failed != 0 || passed == 0 || total != passed + skipped {
        return Err(ValidationError::new(format!(
            "dotnet test summary is not a non-vacuous success: {summary}"
        )));
    }
    Ok(passed)
}

fn labeled_dotnet_count(summary: &str, label: &str) -> Result<usize, ValidationError> {
    let tail = summary
        .split_once(label)
        .map(|(_, tail)| tail)
        .ok_or_else(|| ValidationError::new(format!("dotnet test summary missing {label}")))?;
    let digits = tail
        .trim_start()
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(ValidationError::new(format!(
            "dotnet test summary has invalid {label} count"
        )));
    }
    digits.parse::<usize>().map_err(|error| {
        ValidationError::new(format!(
            "dotnet test summary has invalid {label} count: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_parser_requires_one_consistent_nonzero_success() {
        let success = b"Passed!  - Failed:     0, Passed:    72, Skipped:     0, Total:    72\n";
        assert_eq!(dotnet_passed_test_count(success, b"").unwrap(), 72);

        for invalid in [
            b"Build succeeded.\n".as_slice(),
            b"Passed! - Failed: 0, Passed: 0, Skipped: 0, Total: 0\n".as_slice(),
            b"Passed! - Failed: 1, Passed: 71, Skipped: 0, Total: 72\n".as_slice(),
            b"Passed! - Failed: 0, Passed: 72, Skipped: 1, Total: 72\n".as_slice(),
        ] {
            assert!(dotnet_passed_test_count(invalid, b"").is_err());
        }
        let duplicate = [success.as_slice(), success.as_slice()].concat();
        assert!(dotnet_passed_test_count(&duplicate, b"").is_err());
    }
}
