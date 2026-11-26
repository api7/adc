//! Black-box coverage for environment-variable warnings from the real CLI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("adc-env-warning-{}-{unique}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn warns_once_per_source_file_and_not_for_an_explicitly_empty_variable() {
    let test_dir = TestDir::new();
    let first_file = test_dir.path().join("first.yaml");
    let second_file = test_dir.path().join("second.yaml");
    fs::write(
        &first_file,
        "services:\n  - name: service-a-${ADC_E2E_MISSING}\n",
    )
    .unwrap();
    fs::write(
        &second_file,
        "services:\n  - name: service-b-${ADC_E2E_MISSING}-${ADC_E2E_EMPTY}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_adc"))
        .args([
            "lint",
            "-f",
            first_file.to_str().unwrap(),
            "-f",
            second_file.to_str().unwrap(),
        ])
        .env_remove("ADC_E2E_MISSING")
        .env("ADC_E2E_EMPTY", "")
        .output()
        .expect("failed to run the adc binary");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{combined}");
    assert_eq!(combined.matches("ADC_E2E_MISSING").count(), 2, "{combined}");
    assert!(
        combined.contains(&first_file.display().to_string()),
        "{combined}"
    );
    assert!(
        combined.contains(&second_file.display().to_string()),
        "{combined}"
    );
    assert!(!combined.contains("ADC_E2E_EMPTY"), "{combined}");
}
