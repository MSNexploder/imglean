use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::image::ImageFormat;
use crate::limits::{DEFAULT_STRATEGY_TIMEOUT, PROVIDER_DISCOVERY_TIMEOUT};
use crate::process;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StrategyId {
    OxipngLibdeflateV1,
    OxipngZopfliV1,
    OptipngV1,
    PngquantV1,
    JpegtranV1,
    MozjpegV1,
    JpegliV1,
}

impl StrategyId {
    pub const ALL: [Self; 7] = [
        Self::OxipngLibdeflateV1,
        Self::OxipngZopfliV1,
        Self::OptipngV1,
        Self::PngquantV1,
        Self::JpegtranV1,
        Self::MozjpegV1,
        Self::JpegliV1,
    ];

    pub const EMBEDDED: [Self; 2] = [Self::OxipngLibdeflateV1, Self::OxipngZopfliV1];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OxipngLibdeflateV1 => "oxipng-libdeflate-v1",
            Self::OxipngZopfliV1 => "oxipng-zopfli-v1",
            Self::OptipngV1 => "optipng-v1",
            Self::PngquantV1 => "pngquant-v1",
            Self::JpegtranV1 => "jpegtran-v1",
            Self::MozjpegV1 => "mozjpeg-v1",
            Self::JpegliV1 => "jpegli-v1",
        }
    }

    pub const fn format(self) -> ImageFormat {
        match self {
            Self::OxipngLibdeflateV1
            | Self::OxipngZopfliV1
            | Self::OptipngV1
            | Self::PngquantV1 => ImageFormat::Png,
            Self::JpegtranV1 | Self::MozjpegV1 | Self::JpegliV1 => ImageFormat::Jpeg,
        }
    }

    pub const fn needs_numeric_quality(self) -> bool {
        matches!(self, Self::PngquantV1 | Self::MozjpegV1 | Self::JpegliV1)
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
    Pngquant,
    Jpegtran,
    Mozjpeg,
    Jpegli,
}

impl ProviderId {
    pub const ALL: [Self; 5] = [
        Self::Optipng,
        Self::Pngquant,
        Self::Jpegtran,
        Self::Mozjpeg,
        Self::Jpegli,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "optipng" => Some(Self::Optipng),
            "pngquant" => Some(Self::Pngquant),
            "jpegtran" => Some(Self::Jpegtran),
            "mozjpeg" => Some(Self::Mozjpeg),
            "jpegli" => Some(Self::Jpegli),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Optipng => "optipng",
            Self::Pngquant => "pngquant",
            Self::Jpegtran => "jpegtran",
            Self::Mozjpeg => "mozjpeg",
            Self::Jpegli => "jpegli",
        }
    }

    pub const fn strategy(self) -> StrategyId {
        match self {
            Self::Optipng => StrategyId::OptipngV1,
            Self::Pngquant => StrategyId::PngquantV1,
            Self::Jpegtran => StrategyId::JpegtranV1,
            Self::Mozjpeg => StrategyId::MozjpegV1,
            Self::Jpegli => StrategyId::JpegliV1,
        }
    }

    const fn executable_name(self) -> &'static str {
        match self {
            Self::Optipng => "optipng",
            Self::Pngquant => "pngquant",
            Self::Jpegtran => "jpegtran",
            Self::Mozjpeg => "cjpeg",
            Self::Jpegli => "cjpegli",
        }
    }

    const fn probe_argument(self) -> &'static str {
        match self {
            Self::Optipng => "-help",
            Self::Pngquant | Self::Jpegli => "--help",
            Self::Jpegtran | Self::Mozjpeg => "-help",
        }
    }

    const fn capability_markers(self) -> &'static [&'static str] {
        match self {
            Self::Optipng => &["optipng", "[options]", "-strip"],
            Self::Pngquant => &["pngquant", "--quality", "--strip"],
            Self::Jpegtran => &[
                "usage:",
                "-copy none",
                "-copy all",
                "-optimize",
                "-progressive",
                "-outfile",
                "-strict",
            ],
            Self::Mozjpeg => &[
                "-quality",
                "-progressive",
                "-optimize",
                "-strict",
                "-outfile",
                "-revert",
            ],
            Self::Jpegli => &[
                "input can be",
                "compressed jpeg output file",
                "--quality",
                "--progressive_level",
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Quality {
    #[default]
    Lossless,
    Numeric(u8),
}

impl Quality {
    pub const fn numeric(self) -> Option<u8> {
        match self {
            Self::Lossless => None,
            Self::Numeric(value) => Some(value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderPath {
    pub provider: ProviderId,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub disabled: Vec<StrategyId>,
    pub required: Vec<StrategyId>,
    pub provider_paths: Vec<ProviderPath>,
    pub quality: Quality,
    pub timeout: Duration,
    pub strip_metadata: bool,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            disabled: Vec::new(),
            required: Vec::new(),
            provider_paths: Vec::new(),
            quality: Quality::default(),
            timeout: DEFAULT_STRATEGY_TIMEOUT,
            strip_metadata: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Strategy {
    pub id: StrategyId,
    pub execution: Execution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    pub id: StrategyId,
    pub state: RegistryState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryState {
    Runnable(Execution),
    Disabled,
    Unavailable,
    NotApplicable,
}

impl fmt::Display for Strategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Execution {
    Embedded,
    External { executable: PathBuf },
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

pub fn resolve(selection: &Selection) -> Result<Vec<RegistryEntry>, DiscoveryError> {
    let mut registry = StrategyId::EMBEDDED
        .into_iter()
        .map(|id| RegistryEntry {
            id,
            state: if selection.disabled.contains(&id) {
                RegistryState::Disabled
            } else {
                RegistryState::Runnable(Execution::Embedded)
            },
        })
        .collect::<Vec<_>>();

    for provider in ProviderId::ALL {
        let id = provider.strategy();
        let state = if selection.disabled.contains(&id) {
            RegistryState::Disabled
        } else if id.needs_numeric_quality() && selection.quality == Quality::Lossless {
            RegistryState::NotApplicable
        } else {
            let configured = selection
                .provider_paths
                .iter()
                .find(|path| path.provider == provider)
                .map(|path| path.path.as_path());
            let required = selection.required.contains(&id);
            match discover(provider, configured) {
                Ok(Some(execution)) => RegistryState::Runnable(execution),
                Ok(None) if required => {
                    return discovery(&format!("required strategy {id} is unavailable"));
                }
                Err(error) if required => return Err(error),
                Ok(None) | Err(_) => RegistryState::Unavailable,
            }
        };
        registry.push(RegistryEntry { id, state });
    }

    for required in &selection.required {
        if registry.iter().any(|entry| {
            entry.id == *required && matches!(entry.state, RegistryState::NotApplicable)
        }) {
            return discovery(&format!(
                "required strategy {required} is not applicable at lossless quality"
            ));
        }
        if !registry
            .iter()
            .any(|entry| entry.id == *required && matches!(entry.state, RegistryState::Runnable(_)))
        {
            return discovery(&format!("required strategy {required} is unavailable"));
        }
    }
    Ok(registry)
}

fn discover(
    provider: ProviderId,
    configured: Option<&Path>,
) -> Result<Option<Execution>, DiscoveryError> {
    if let Some(path) = configured {
        let path = resolve_executable(path)?;
        probe(provider, &path)?;
        return Ok(Some(Execution::External { executable: path }));
    }
    find_compatible_on_path(provider)
}

fn probe(provider: ProviderId, path: &Path) -> Result<(), DiscoveryError> {
    let mut command = Command::new(path);
    command.arg(provider.probe_argument());
    let output = process::run(command, PROVIDER_DISCOVERY_TIMEOUT).map_err(|()| {
        error(&format!(
            "cannot start the {} capability probe",
            provider.as_str()
        ))
    })?;
    if output.timed_out {
        return discovery(&format!("{} capability probe timed out", provider.as_str()));
    }
    let accepted_status = output.status.is_some_and(|status| {
        status.success()
            || (matches!(provider, ProviderId::Jpegtran | ProviderId::Mozjpeg)
                && status.code() == Some(1))
    });
    if !accepted_status {
        return discovery(&format!("{} capability probe failed", provider.as_str()));
    }
    if output.stdout.truncated || output.stderr.truncated {
        return discovery(&format!(
            "{} capability output exceeded the diagnostic limit",
            provider.as_str()
        ));
    }
    let mut bytes = output.stdout.bytes;
    bytes.extend_from_slice(&output.stderr.bytes);
    let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    if !has_capabilities(provider, &text) {
        return discovery(&format!(
            "{} executable does not expose the required CLI capabilities",
            provider.as_str()
        ));
    }
    Ok(())
}

fn has_capabilities(provider: ProviderId, text: &str) -> bool {
    provider
        .capability_markers()
        .iter()
        .all(|marker| text.contains(marker))
        && (provider != ProviderId::Jpegli
            || text.lines().any(|line| {
                line.contains("input can be")
                    && line
                        .split(|character: char| !character.is_ascii_alphanumeric())
                        .any(|word| word == "jpeg")
            }))
}

fn find_compatible_on_path(provider: ProviderId) -> Result<Option<Execution>, DiscoveryError> {
    let Some(path) = std::env::var_os("PATH") else {
        return Ok(None);
    };
    let executable = provider.executable_name();
    let executable = if cfg!(windows) {
        format!("{executable}.exe")
    } else {
        executable.to_owned()
    };
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(&executable);
        if candidate.is_file()
            && let Ok(resolved) = resolve_executable(&candidate)
            && probe(provider, &resolved).is_ok()
        {
            return Ok(Some(Execution::External {
                executable: resolved,
            }));
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
    fn strategy_ids_are_stable_unique_and_format_specific() {
        assert_eq!(
            StrategyId::ALL.map(StrategyId::as_str),
            [
                "oxipng-libdeflate-v1",
                "oxipng-zopfli-v1",
                "optipng-v1",
                "pngquant-v1",
                "jpegtran-v1",
                "mozjpeg-v1",
                "jpegli-v1",
            ]
        );
        assert_eq!(StrategyId::MozjpegV1.format(), ImageFormat::Jpeg);
        assert_eq!(StrategyId::JpegtranV1.format(), ImageFormat::Jpeg);
        assert_eq!(StrategyId::OptipngV1.format(), ImageFormat::Png);
    }

    #[test]
    fn lossy_strategies_are_not_applicable_at_lossless_quality() {
        let selection = Selection {
            disabled: vec![StrategyId::OptipngV1],
            ..Selection::default()
        };
        let registry = resolve(&selection).unwrap();
        for id in [
            StrategyId::PngquantV1,
            StrategyId::MozjpegV1,
            StrategyId::JpegliV1,
        ] {
            assert_eq!(
                registry.iter().find(|entry| entry.id == id).unwrap().state,
                RegistryState::NotApplicable
            );
        }
    }

    #[test]
    fn metadata_stripping_keeps_strategies_without_a_native_control_applicable() {
        let selection = Selection {
            quality: Quality::Numeric(80),
            strip_metadata: true,
            disabled: vec![
                StrategyId::OptipngV1,
                StrategyId::PngquantV1,
                StrategyId::JpegtranV1,
                StrategyId::JpegliV1,
            ],
            ..Selection::default()
        };
        let registry = resolve(&selection).unwrap();
        assert!(
            !matches!(
                registry
                    .iter()
                    .find(|entry| entry.id == StrategyId::MozjpegV1)
                    .unwrap()
                    .state,
                RegistryState::NotApplicable
            ),
            "MozJPEG should remain eligible when metadata stripping is requested"
        );
    }

    #[cfg(unix)]
    #[test]
    fn every_configured_provider_is_accepted_by_capability_not_version() {
        let directory = test_directory();
        let fixtures = [
            (
                ProviderId::Optipng,
                "optipng [options] -strip future release",
            ),
            (
                ProviderId::Pngquant,
                "pngquant --quality --strip future release",
            ),
            (
                ProviderId::Jpegtran,
                "usage: provider -copy none -copy all -optimize -progressive -outfile -strict",
            ),
            (
                ProviderId::Mozjpeg,
                "-quality -progressive -optimize -strict -outfile -revert",
            ),
            (
                ProviderId::Jpegli,
                "input can be PPM, PNG, APNG, JPEG; compressed JPEG output file; \
                 --quality --progressive_level future release",
            ),
        ];
        for (provider, output) in fixtures {
            let executable = directory.join(provider.as_str());
            let exit = if matches!(provider, ProviderId::Jpegtran | ProviderId::Mozjpeg) {
                "exit 1"
            } else {
                ""
            };
            write_executable(
                &executable,
                &format!("#!/bin/sh\nprintf '%s\\n' '{output}'\n{exit}\n"),
            );
            let selection = Selection {
                required: vec![provider.strategy()],
                provider_paths: vec![ProviderPath {
                    provider,
                    path: executable.clone(),
                }],
                quality: Quality::Numeric(80),
                disabled: ProviderId::ALL
                    .into_iter()
                    .filter(|candidate| *candidate != provider)
                    .map(ProviderId::strategy)
                    .collect(),
                timeout: DEFAULT_STRATEGY_TIMEOUT,
                strip_metadata: false,
            };
            let registry = resolve(&selection).unwrap();
            assert_eq!(
                registry
                    .iter()
                    .find(|entry| entry.id == provider.strategy())
                    .unwrap()
                    .state,
                RegistryState::Runnable(Execution::External {
                    executable: fs::canonicalize(executable).unwrap(),
                })
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn wrong_executable_identity_is_optional_unless_required() {
        let directory = test_directory();
        let executable = directory.join("cjpeg");
        write_executable(&executable, "#!/bin/sh\nprintf 'libjpeg-turbo version 9'\n");
        let configured = ProviderPath {
            provider: ProviderId::Mozjpeg,
            path: executable,
        };
        let optional = Selection {
            provider_paths: vec![configured.clone()],
            quality: Quality::Numeric(80),
            disabled: vec![
                StrategyId::OptipngV1,
                StrategyId::PngquantV1,
                StrategyId::JpegtranV1,
                StrategyId::JpegliV1,
            ],
            ..Selection::default()
        };
        assert_eq!(
            resolve(&optional)
                .unwrap()
                .iter()
                .find(|entry| entry.id == StrategyId::MozjpegV1)
                .unwrap()
                .state,
            RegistryState::Unavailable
        );

        let required = Selection {
            required: vec![StrategyId::MozjpegV1],
            provider_paths: vec![configured],
            quality: Quality::Numeric(80),
            disabled: optional.disabled,
            ..Selection::default()
        };
        assert!(
            resolve(&required)
                .unwrap_err()
                .message()
                .contains("does not expose the required CLI capabilities")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn incompatible_jpegtran_is_optional_unless_required() {
        let directory = test_directory();
        let executable = directory.join("jpegtran");
        write_executable(
            &executable,
            "#!/bin/sh\nprintf '%s\n' \
             'usage: provider -optimize -progressive -outfile -strict'\nexit 1\n",
        );
        let configured = ProviderPath {
            provider: ProviderId::Jpegtran,
            path: executable,
        };
        let optional = Selection {
            provider_paths: vec![configured.clone()],
            disabled: vec![
                StrategyId::OptipngV1,
                StrategyId::MozjpegV1,
                StrategyId::JpegliV1,
            ],
            ..Selection::default()
        };
        assert_eq!(
            resolve(&optional)
                .unwrap()
                .iter()
                .find(|entry| entry.id == StrategyId::JpegtranV1)
                .unwrap()
                .state,
            RegistryState::Unavailable
        );

        let required = Selection {
            required: vec![StrategyId::JpegtranV1],
            provider_paths: vec![configured],
            disabled: optional.disabled,
            ..Selection::default()
        };
        assert!(
            resolve(&required)
                .unwrap_err()
                .message()
                .contains("does not expose the required CLI capabilities")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn jpegli_without_jpeg_input_is_unavailable() {
        let directory = test_directory();
        let executable = directory.join("cjpegli");
        write_executable(
            &executable,
            "#!/bin/sh\nprintf '%s\n' 'input can be PPM, PNG' \
             'compressed JPEG output file; --quality --progressive_level'\n",
        );
        let selection = Selection {
            provider_paths: vec![ProviderPath {
                provider: ProviderId::Jpegli,
                path: executable,
            }],
            quality: Quality::Numeric(80),
            disabled: vec![
                StrategyId::OptipngV1,
                StrategyId::PngquantV1,
                StrategyId::JpegtranV1,
                StrategyId::MozjpegV1,
            ],
            ..Selection::default()
        };

        assert_eq!(
            resolve(&selection)
                .unwrap()
                .iter()
                .find(|entry| entry.id == StrategyId::JpegliV1)
                .unwrap()
                .state,
            RegistryState::Unavailable
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
