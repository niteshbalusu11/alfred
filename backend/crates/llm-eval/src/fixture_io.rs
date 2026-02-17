use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::assistant_case::AssistantRoutingEvalCaseFixture;
use crate::case::EvalCaseFixture;

#[derive(Debug, Error)]
pub enum FixtureIoError {
    #[error("failed to read fixtures directory {path}: {source}")]
    ReadDir {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read fixture file {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("fixture file {path} is not valid JSON: {source}")]
    ParseJson {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to write fixture file {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to encode JSON payload for {path}: {source}")]
    EncodeJson {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

pub fn load_cases() -> Result<Vec<EvalCaseFixture>, FixtureIoError> {
    let mut files = list_case_files("cases")?;
    files.sort();

    let mut cases = Vec::with_capacity(files.len());
    for file in files {
        let raw = fs::read_to_string(&file).map_err(|source| FixtureIoError::ReadFile {
            path: file.display().to_string(),
            source,
        })?;
        let case = serde_json::from_str::<EvalCaseFixture>(&raw).map_err(|source| {
            FixtureIoError::ParseJson {
                path: file.display().to_string(),
                source,
            }
        })?;
        cases.push(case);
    }

    Ok(cases)
}

pub fn load_assistant_routing_cases() -> Result<Vec<AssistantRoutingEvalCaseFixture>, FixtureIoError>
{
    let mut files = list_case_files("assistant_cases")?;
    files.sort();

    let mut cases = Vec::with_capacity(files.len());
    for file in files {
        let raw = fs::read_to_string(&file).map_err(|source| FixtureIoError::ReadFile {
            path: file.display().to_string(),
            source,
        })?;
        let case =
            serde_json::from_str::<AssistantRoutingEvalCaseFixture>(&raw).map_err(|source| {
                FixtureIoError::ParseJson {
                    path: file.display().to_string(),
                    source,
                }
            })?;
        cases.push(case);
    }

    Ok(cases)
}

pub fn golden_path(case_id: &str) -> PathBuf {
    fixture_root()
        .join("goldens")
        .join(format!("{case_id}.golden.json"))
}

pub fn read_json_value(path: &Path) -> Result<Value, FixtureIoError> {
    let raw = fs::read_to_string(path).map_err(|source| FixtureIoError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| FixtureIoError::ParseJson {
        path: path.display().to_string(),
        source,
    })
}

pub fn write_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<(), FixtureIoError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| FixtureIoError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let mut encoded =
        serde_json::to_string_pretty(value).map_err(|source| FixtureIoError::EncodeJson {
            path: path.display().to_string(),
            source,
        })?;
    encoded.push('\n');

    fs::write(path, encoded).map_err(|source| FixtureIoError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn list_case_files(directory_name: &str) -> Result<Vec<PathBuf>, FixtureIoError> {
    let cases_dir = fixture_root().join(directory_name);
    let entries = fs::read_dir(&cases_dir).map_err(|source| FixtureIoError::ReadDir {
        path: cases_dir.display().to_string(),
        source,
    })?;

    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| FixtureIoError::ReadDir {
            path: cases_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        }
    }

    Ok(files)
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}
