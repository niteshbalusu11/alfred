use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalMode {
    Mocked,
    Live,
}

impl EvalMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mocked => "mocked",
            Self::Live => "live",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub mode: EvalMode,
    pub update_goldens: bool,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("unknown argument: {0}")]
    UnknownArgument(String),
    #[error("missing value for argument: {0}")]
    MissingValue(String),
    #[error("invalid --mode value: {0}")]
    InvalidMode(String),
    #[error("--update-goldens is only supported in mocked mode")]
    UpdateGoldensRequiresMockedMode,
    #[error("help requested")]
    HelpRequested,
}

impl CliOptions {
    pub fn parse<I>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut mode = EvalMode::Mocked;
        let mut update_goldens = false;

        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--help" | "-h" => return Err(CliError::HelpRequested),
                "--mode" => {
                    let value = iter.next().ok_or(CliError::MissingValue(arg.clone()))?;
                    mode = parse_mode(&value)?;
                }
                "--update-goldens" => update_goldens = true,
                unknown => return Err(CliError::UnknownArgument(unknown.to_string())),
            }
        }

        if update_goldens && mode != EvalMode::Mocked {
            return Err(CliError::UpdateGoldensRequiresMockedMode);
        }

        Ok(Self {
            mode,
            update_goldens,
        })
    }
}

fn parse_mode(value: &str) -> Result<EvalMode, CliError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "mocked" => Ok(EvalMode::Mocked),
        "live" => Ok(EvalMode::Live),
        _ => Err(CliError::InvalidMode(value.to_string())),
    }
}
