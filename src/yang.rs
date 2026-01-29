use std::fs;
use zed_extension_api::{self as zed, settings::LspSettings, serde_json, LanguageServerId, Result};

struct YangExtension {
    cached_binary_path: Option<String>,
}

impl YangExtension {
    const LANGUAGE_SERVER_ID: &'static str = "yang-lsp";

    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
    ) -> Result<String> {
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            "TypeFox/yang-lsp",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let version = release.version.trim_start_matches('v');
        let download_dir = format!("yang-lsp-{version}");
        let inner_dir = format!("yang-language-server-{version}");

        let (os, _) = zed::current_platform();
        let binary_path = format!(
            "{download_dir}/{inner_dir}/bin/yang-language-server{extension}",
            extension = match os {
                zed::Os::Mac | zed::Os::Linux => "",
                zed::Os::Windows => ".bat",
            }
        );

        if fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            self.cached_binary_path = Some(binary_path.clone());
            return Ok(binary_path);
        }

        let asset = release
            .assets
            .iter()
            .find(|asset| {
                asset.name.starts_with("yang-language-server_")
                    && asset.name.ends_with(".zip")
                    && !asset.name.contains("diagram")
            })
            .ok_or("No yang-language-server zip asset found in release")?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );

        zed::download_file(
            &asset.download_url,
            &download_dir,
            zed::DownloadedFileType::Zip,
        )
        .map_err(|e| format!("Failed to download yang-lsp: {e}"))?;

        zed::make_file_executable(&binary_path)
            .map_err(|e| format!("Failed to make binary executable: {e}"))?;

        self.cleanup_old_versions(&download_dir);
        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }

    fn cleanup_old_versions(&self, current_dir: &str) {
        let Ok(entries) = fs::read_dir(".") else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("yang-lsp-") && name.as_ref() != current_dir {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
}

impl zed::Extension for YangExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        match language_server_id.as_ref() {
            Self::LANGUAGE_SERVER_ID => {
                let binary_path = self.language_server_binary_path(language_server_id)?;
                Ok(zed::Command {
                    command: binary_path,
                    args: vec![],
                    env: Default::default(),
                })
            }
            _ => Err(format!("Unknown language server: {language_server_id}")),
        }
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        if language_server_id.as_ref() != Self::LANGUAGE_SERVER_ID {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.settings.clone())
            .unwrap_or_default();

        Ok(Some(settings))
    }
}

zed::register_extension!(YangExtension);
