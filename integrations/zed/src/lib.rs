use zed_extension_api::{
    self as zed, serde_json, settings::ContextServerSettings, Architecture, Command,
    ContextServerConfiguration, ContextServerId, DownloadedFileType, GithubReleaseOptions, Os,
    Project, Result,
};

const GITHUB_REPO: &str = "annibale-x/webshift";
const BINARY_NAME: &str = "mcp-webshift";
const CONTEXT_SERVER_ID: &str = "mcp-webshift";

struct WebshiftExtension {
    cached_binary_path: Option<String>,
}

impl WebshiftExtension {
    /// Download (or reuse) the platform-native binary from the latest GitHub release.
    fn binary_path(&mut self) -> Result<String> {
        // Fast path: cached in memory and still on disk.
        if let Some(path) = &self.cached_binary_path {
            if std::fs::metadata(path).map_or(false, |m| m.is_file()) {
                return Ok(path.clone());
            }
        }

        let release = zed::latest_github_release(
            GITHUB_REPO,
            GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (os, arch) = zed::current_platform();
        let is_windows = matches!(os, Os::Windows);

        let target = match (os, arch) {
            (Os::Mac, Architecture::Aarch64) => "aarch64-apple-darwin",
            (Os::Mac, Architecture::X8664) => "x86_64-apple-darwin",
            (Os::Linux, Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
            (Os::Linux, Architecture::X8664) => "x86_64-unknown-linux-gnu",
            (Os::Windows, Architecture::X8664) => "x86_64-pc-windows-msvc",
            _ => return Err(format!("unsupported platform: {os:?}/{arch:?}")),
        };

        let asset_name = if is_windows {
            format!("{BINARY_NAME}-{target}.exe")
        } else {
            format!("{BINARY_NAME}-{target}")
        };

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| {
                format!(
                    "asset '{}' not found in release {}",
                    asset_name, release.version
                )
            })?;

        let binary_path = if is_windows {
            format!("{BINARY_NAME}-{}.exe", release.version)
        } else {
            format!("{BINARY_NAME}-{}", release.version)
        };

        if !std::fs::metadata(&binary_path).map_or(false, |m| m.is_file()) {
            zed::download_file(
                &asset.download_url,
                &binary_path,
                DownloadedFileType::Uncompressed,
            )
            .map_err(|e| format!("failed to download {BINARY_NAME}: {e}"))?;

            if !is_windows {
                zed::make_file_executable(&binary_path)?;
            }
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }

    /// Map the JSON settings object into `mcp-webshift` CLI arguments.
    /// Keys are already CLI flags (e.g. "--default-backend"); values are string, integer, or boolean.
    fn settings_to_args(settings: &serde_json::Value) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(obj) = settings.as_object() {
            for (flag, val) in obj {
                let val_str = match val {
                    serde_json::Value::String(s) if !s.is_empty() => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                args.push(flag.clone());
                args.push(val_str);
            }
        }
        args
    }
}

impl zed::Extension for WebshiftExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn context_server_command(
        &mut self,
        _context_server_id: &ContextServerId,
        project: &Project,
    ) -> Result<Command> {
        let binary_path = self.binary_path()?;

        let settings = ContextServerSettings::for_project(CONTEXT_SERVER_ID, project)?;
        let args = settings
            .settings
            .as_ref()
            .map(|s| Self::settings_to_args(s))
            .unwrap_or_default();

        Ok(Command {
            command: binary_path,
            args,
            env: vec![],
        })
    }

    fn context_server_configuration(
        &mut self,
        _context_server_id: &ContextServerId,
        _project: &Project,
    ) -> Result<Option<ContextServerConfiguration>> {
        Ok(Some(ContextServerConfiguration {
            installation_instructions:
                "mcp-webshift is installed automatically from \
                 [GitHub Releases](https://github.com/annibale-x/webshift/releases).\n\
                 No manual setup required — the native binary is downloaded on first use.\n\n\
                 Keys are [CLI arguments](https://github.com/annibale-x/webshift/blob/main/docs/CONFIGURATION.md#cli-arguments-mcp-server-only)."
                    .to_string(),
            settings_schema: include_str!("../assets/settings_schema.json").to_string(),
            default_settings: include_str!("../assets/default_settings.json").to_string(),
        }))
    }
}

zed::register_extension!(WebshiftExtension);
