use zed_extension_api::{
    self as zed,
    serde_json,
    settings::ContextServerSettings,
    Architecture, Command, ContextServerConfiguration, ContextServerId,
    DownloadedFileType, GithubReleaseOptions, Os, Project, Result,
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
    /// Handles string, integer, and boolean JSON values.
    /// Only non-empty / non-default-zero values are emitted.
    fn settings_to_args(settings: &serde_json::Value) -> Vec<String> {
        let mut args = Vec::new();

        let all_flags: &[(&str, &str)] = &[
            ("default_backend", "--default-backend"),
            ("language", "--language"),
            ("adaptive_budget", "--adaptive-budget"),
            ("blocked_domains", "--blocked-domains"),
            ("allowed_domains", "--allowed-domains"),
            ("max_download_mb", "--max-download-mb"),
            ("max_result_length", "--max-result-length"),
            ("search_timeout", "--search-timeout"),
            ("oversampling_factor", "--oversampling-factor"),
            ("auto_recovery_fetch", "--auto-recovery-fetch"),
            ("max_total_results", "--max-total-results"),
            ("max_query_budget", "--max-query-budget"),
            ("max_search_queries", "--max-search-queries"),
            ("results_per_query", "--results-per-query"),
            ("adaptive_budget_fetch_factor", "--adaptive-budget-fetch-factor"),
            ("searxng_url", "--searxng-url"),
            ("brave_api_key", "--brave-api-key"),
            ("tavily_api_key", "--tavily-api-key"),
            ("exa_api_key", "--exa-api-key"),
            ("serpapi_api_key", "--serpapi-api-key"),
            ("google_api_key", "--google-api-key"),
            ("google_cx", "--google-cx"),
            ("bing_api_key", "--bing-api-key"),
            ("bing_market", "--bing-market"),
            ("llm_enabled", "--llm-enabled"),
            ("llm_base_url", "--llm-base-url"),
            ("llm_api_key", "--llm-api-key"),
            ("llm_model", "--llm-model"),
            ("llm_timeout", "--llm-timeout"),
            ("llm_expansion_enabled", "--llm-expansion-enabled"),
            ("llm_summarization_enabled", "--llm-summarization-enabled"),
            ("llm_rerank_enabled", "--llm-rerank-enabled"),
            ("llm_max_summary_words", "--llm-max-summary-words"),
            ("llm_input_budget_factor", "--llm-input-budget-factor"),
        ];

        for (key, flag) in all_flags {
            let val_str = match settings.get(key) {
                Some(serde_json::Value::String(s)) if !s.is_empty() => s.clone(),
                Some(serde_json::Value::Number(n)) => n.to_string(),
                Some(serde_json::Value::Bool(b)) => b.to_string(),
                _ => continue,
            };
            args.push(flag.to_string());
            args.push(val_str);
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
                 Configure your preferred backend and optional API keys in the settings below."
                    .to_string(),
            settings_schema: include_str!("../assets/settings_schema.json").to_string(),
            default_settings: include_str!("../assets/default_settings.json").to_string(),
        }))
    }
}

zed::register_extension!(WebshiftExtension);
