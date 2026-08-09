use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::limits::PROVIDER_DISCOVERY_TIMEOUT;
use crate::process;

const SUPPORTED_OPTIPNG_VERSION: &str = "7.9.1";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StrategyId {
    OxipngLibdeflateV1,
    OxipngZopfliV1,
    OptipngV1,
}

impl StrategyId {
    pub const ALL: [Self; 3] = [
        Self::OxipngLibdeflateV1,
        Self::OxipngZopfliV1,
        Self::OptipngV1,
    ];

    pub const EMBEDDED: [Self; 2] = [Self::OxipngLibdeflateV1, Self::OxipngZopfliV1];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OxipngLibdeflateV1 => "oxipng-libdeflate-v1",
            Self::OxipngZopfliV1 => "oxipng-zopfli-v1",
            Self::OptipngV1 => "optipng-v1",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|strategy| strategy.as_str() == value)
    }
}

impl fmt::Display for StrategyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProviderId {
    Optipng,
}

impl ProviderId {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "optipng" => Some(Self::Optipng),
            _ => None,
        }
    }

    pub const fn strategy(self) -> StrategyId {
        match self {
            Self::Optipng => StrategyId::OptipngV1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderPath {
    pub provider: ProviderId,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    pub disabled: Vec<StrategyId>,
    pub required: Vec<StrategyId>,
    pub provider_paths: Vec<ProviderPath>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Strategy {
    pub id: StrategyId,
    pub execution: Execution,
}

impl fmt::Display for Strategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Execution {
    Embedded,
    External {
        executable: PathBuf,
        version: String,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub struct DiscoveryError {
    message: String,
}

impl DiscoveryError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn resolve(selection: &Selection) -> Result<Vec<Strategy>, DiscoveryError> {
    let mut strategies = StrategyId::EMBEDDED
        .into_iter()
        .filter(|id| !selection.disabled.contains(id))
        .map(|id| Strategy {
            id,
            execution: Execution::Embedded,
        })
        .collect::<Vec<_>>();

    let optipng_id = StrategyId::OptipngV1;
    if !selection.disabled.contains(&optipng_id) {
        let configured = selection
            .provider_paths
            .iter()
            .find(|path| path.provider == ProviderId::Optipng)
            .map(|path| path.path.as_path());
        let required = selection.required.contains(&optipng_id);
        match discover_optipng(configured) {
            Ok(Some(execution)) => strategies.push(Strategy {
                id: optipng_id,
                execution,
            }),
            Ok(None) if required => {
                return discovery("required strategy optipng-v1 is unavailable");
            }
            Err(error) if required => return Err(error),
            Ok(None) | Err(_) => {}
        }
    }

    for required in &selection.required {
        if !strategies.iter().any(|strategy| strategy.id == *required) {
            return discovery(&format!("required strategy {required} is unavailable"));
        }
    }
    Ok(strategies)
}

fn discover_optipng(configured: Option<&Path>) -> Result<Option<Execution>, DiscoveryError> {
    let path = match configured {
        Some(path) => resolve_executable(path).map(Some),
        None => find_on_path(optipng_executable_name()),
    }?;
    let Some(path) = path else {
        return Ok(None);
    };
    let mut command = Command::new(&path);
    command.arg("-version");
    let output = process::run(command, PROVIDER_DISCOVERY_TIMEOUT)
        .map_err(|()| error("cannot start the OptiPNG version probe"))?;
    if output.timed_out {
        return discovery("OptiPNG version probe timed out");
    }
    if output.status.is_none_or(|status| !status.success()) {
        return discovery("OptiPNG version probe failed");
    }
    if output.stdout.truncated || output.stderr.truncated {
        return discovery("OptiPNG version output exceeded the diagnostic limit");
    }
    let version = parse_optipng_version(&output.stdout.bytes)
        .or_else(|| parse_optipng_version(&output.stderr.bytes))
        .ok_or_else(|| error("cannot parse the OptiPNG version"))?;
    if version != SUPPORTED_OPTIPNG_VERSION {
        return discovery(&format!(
            "unsupported OptiPNG version {version}; expected {SUPPORTED_OPTIPNG_VERSION}"
        ));
    }
    Ok(Some(Execution::External {
        executable: path,
        version: version.to_owned(),
    }))
}

fn find_on_path(executable: &OsStr) -> Result<Option<PathBuf>, DiscoveryError> {
    let Some(path) = std::env::var_os("PATH") else {
        return Ok(None);
    };
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(executable);
        if candidate.is_file()
            && let Ok(resolved) = resolve_executable(&candidate)
        {
            return Ok(Some(resolved));
        }
    }
    Ok(None)
}

fn resolve_executable(path: &Path) -> Result<PathBuf, DiscoveryError> {
    let resolved = fs::canonicalize(path)
        .map_err(|_| error("cannot resolve the configured provider executable"))?;
    let metadata = fs::metadata(&resolved)
        .map_err(|_| error("cannot inspect the configured provider executable"))?;
    if !metadata.is_file() {
        return discovery("configured provider path is not a regular file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o111 == 0 {
            return discovery("configured provider file is not executable");
        }
    }
    Ok(resolved)
}

fn optipng_executable_name() -> &'static OsStr {
    if cfg!(windows) {
        OsStr::new("optipng.exe")
    } else {
        OsStr::new("optipng")
    }
}

fn parse_optipng_version(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    let marker = "OptiPNG version ";
    let start = text.find(marker)? + marker.len();
    text[start..].split_whitespace().next().map(|version| {
        version.trim_end_matches(|character: char| {
            !character.is_ascii_alphanumeric() && character != '.'
        })
    })
}

fn discovery<T>(message: &str) -> Result<T, DiscoveryError> {
    Err(error(message))
}

fn error(message: &str) -> DiscoveryError {
    DiscoveryError {
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn strategy_ids_are_stable_and_unique() {
        assert_eq!(
            StrategyId::ALL.map(StrategyId::as_str),
            ["oxipng-libdeflate-v1", "oxipng-zopfli-v1", "optipng-v1"]
        );
    }

    #[test]
    fn parses_supported_optipng_version_output() {
        assert_eq!(
            parse_optipng_version(b"OptiPNG version 7.9.1\nCopyright"),
            Some("7.9.1")
        );
        assert_eq!(parse_optipng_version(b"not optipng"), None);
    }

    #[test]
    fn provider_version_record_matches_the_adapter() {
        assert_eq!(
            include_str!("../ci/optipng-version.txt").trim(),
            SUPPORTED_OPTIPNG_VERSION
        );
    }

    #[test]
    fn embedded_strategies_are_enabled_unless_disabled() {
        let selection = Selection {
            disabled: vec![StrategyId::OxipngZopfliV1, StrategyId::OptipngV1],
            ..Selection::default()
        };
        let strategies = resolve(&selection).unwrap();
        assert_eq!(
            strategies,
            [Strategy {
                id: StrategyId::OxipngLibdeflateV1,
                execution: Execution::Embedded,
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn configured_provider_is_resolved_and_version_checked() {
        let directory = test_directory();
        let executable = directory.join("optipng");
        write_executable(
            &executable,
            "#!/bin/sh\nprintf 'OptiPNG version 7.9.1\\n'\n",
        );
        let selection = Selection {
            required: vec![StrategyId::OptipngV1],
            provider_paths: vec![ProviderPath {
                provider: ProviderId::Optipng,
                path: executable.clone(),
            }],
            ..Selection::default()
        };

        let strategies = resolve(&selection).unwrap();
        assert_eq!(strategies.len(), 3);
        assert_eq!(strategies[2].id, StrategyId::OptipngV1);
        assert_eq!(
            strategies[2].execution,
            Execution::External {
                executable: fs::canonicalize(executable).unwrap(),
                version: "7.9.1".to_owned(),
            }
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn incompatible_provider_is_optional_unless_required() {
        let directory = test_directory();
        let executable = directory.join("optipng");
        write_executable(
            &executable,
            "#!/bin/sh\nprintf 'OptiPNG version 7.9.0\\n'\n",
        );
        let configured = ProviderPath {
            provider: ProviderId::Optipng,
            path: executable,
        };
        let optional = Selection {
            provider_paths: vec![configured.clone()],
            ..Selection::default()
        };
        assert_eq!(resolve(&optional).unwrap().len(), 2);

        let required = Selection {
            required: vec![StrategyId::OptipngV1],
            provider_paths: vec![configured],
            ..Selection::default()
        };
        assert!(
            resolve(&required)
                .unwrap_err()
                .message()
                .contains("unsupported OptiPNG version 7.9.0")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) {
        use std::os::unix::fs::PermissionsExt as _;

        fs::write(path, contents).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(unix)]
    fn test_directory() -> PathBuf {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "imglean-strategy-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
}
