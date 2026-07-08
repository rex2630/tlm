use crate::ui::LaunchUI;
use anyhow::{Context, Result, anyhow, bail};
use bytes::{Buf, Bytes};
use clap::Parser;
use flate2::bufread::GzDecoder;
use reqwest::Url;
use std::{
    env,
    fmt::Display,
    fs::{self, File},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use tar::Archive;
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

const XIVLAUNCHER_BIN_FILENAME: &str = "TruckersMP-Launcher";
const XIVLAUNCHER_VERSION_REMOTE_FILENAME: &str = "version";
const XIVLAUNCHER_VERSIONDATA_LOCAL_FILENAME: &str = "versiondata";
const ARIA2C_BIN_FILENAME: &str = "aria2c";
const EMBEDDED_ARIA2C_TARBALL: &[u8] = include_bytes!("../../static/aria2c-static.tar.gz");

const DISCORD_BRIDGE_FILENAME: &str = "winediscordipcbridge.exe";
const DISCORD_BRIDGE_PREFIX_DIRNAME: &str = "discord-bridge-prefix";

/// Install or update TruckersMP Launcher and then open it.
#[derive(Debug, Clone, Parser)]
pub struct LaunchCommand {
    /// The name of the release tar.gz archive that contains a self-contained TruckersMP Launcher.
    #[clap(
        default_value = "TruckersMP-Launcher.tar.gz",
        long = "release-asset",
        env = "TLM_RELEASE_ASSET"
    )]
    xlcore_release_asset: String,

    /// The URL to a release of TruckersMP Launcher.
    ///
    /// This should be a URL base that contains the following under it:
    ///
    /// - A plaintext file named `version` that contains only a version number.
    ///
    /// - A tar.gz archive with the name of `--release-asset` that contains TruckersMP Launcher files.
    #[clap(
        long = "web-release-url",
        alias = "web-release-url-base",
        env = "TLM_WEB_RELEASE_URL",
        default_value = "https://files.launcher.truckersmp.com/truckersmp-launcher/linux/x64/"
    )]
    xlcore_web_release_url: Url,

    /// The path to where TruckersMP Launcher should be installed.
    #[clap(long = "install-directory", env = "TLM_INSTALL_DIRECTORY", default_value = dirs::data_local_dir().unwrap().join("launcher").into_os_string())]
    xlcore_install_directory: PathBuf,

    /// Source of an aria2c tarball containing a statically compiled `aria2c` binary.
    /// By default an embedded tarball will be used.
    ///
    /// The supported source types are `file:path`, `url:url` or `embedded`.
    #[clap(long = "aria-source", env = "TLM_ARIA_SOURCE", default_value_t = AriaSource::Embedded)]
    aria_source: AriaSource,

    /// Skip checking for & installing new TruckersMP Launcher versions.
    ///
    /// Note: this will not prevent TruckersMP Launcher from installing when not present.
    #[clap(long = "skip-update", env = "TLM_SKIP_UPDATE")]
    skip_update: bool,
}

impl LaunchCommand {
    pub async fn run(self) -> anyhow::Result<()> {
        info!("Attempting launch with: {self:?}");

        // Query the GitHub API or Web Release URL for release information.
        let release = ReleaseAssetInfo::from_url(
                    self.xlcore_web_release_url,
                    &self.xlcore_release_asset,
                    XIVLAUNCHER_VERSION_REMOTE_FILENAME,
                )
                .await?;

        // Conditionally run update check/install depending on flags and versions.
        let xl_installed =
            fs::exists(self.xlcore_install_directory.join(XIVLAUNCHER_BIN_FILENAME))?;
        if xl_installed && self.skip_update {
            info!(
                "TruckersMP-Launcher already installed & version checks are disabled, skipping the update process"
            );
        } else {
            match fs::read_to_string(
                self.xlcore_install_directory
                    .join(XIVLAUNCHER_VERSIONDATA_LOCAL_FILENAME),
            ) {
                Ok(local_ver) => {
                    if xl_installed && local_ver == release.version {
                        info!(
                            "TruckersMP-Launcher is up to date (local: {local_ver} == remote: {})",
                            release.version
                        );
                    } else {
                        let launch_ui = LaunchUI::new();
                        info!(
                            "TruckersMP-Launcher is out of date or missing files (local {local_ver} != remote: {}, bin present: {xl_installed}) - starting update",
                            release.version
                        );
                        install_or_update_xlcore(
                            release,
                            self.aria_source,
                            &self.xlcore_install_directory,
                            true,
                            |txt| {
                                if let Some(ui) = launch_ui.as_ref() {
                                    debug!("Setting progress text to '{txt}'");
                                    ui.set_progress_text(txt)
                                } else {
                                    info!("Progress state: {txt}");
                                }
                            },
                        )
                        .await?;
                        info!("Successfully updated TruckersMP-Launcher to the latest version")
                    }
                }
                Err(err) => {
                    if err.kind() == ErrorKind::NotFound {
                        let launch_ui = LaunchUI::new();
                        info!(
                            "Unable to obtain local version data for TruckersMP-Launcher - installing latest release"
                        );
                        install_or_update_xlcore(
                            release,
                            self.aria_source,
                            &self.xlcore_install_directory,
                            false,
                            |txt| {
                                if let Some(ui) = launch_ui.as_ref() {
                                    debug!("Setting progress text to '{txt}'");
                                    ui.set_progress_text(txt)
                                } else {
                                    info!("Progress state: {txt}");
                                }
                            },
                        )
                        .await?;
                        info!("Successfully installed TruckersMP-Launcher");
                    } else {
                        error!("Something went wrong whilst checking for TruckersMP-Launcher: {err:?}",);
                    }
                }
            };
        }

        let mut discord_bridge = start_discord_bridge(&self.xlcore_install_directory).await;

        info!("Starting TruckersMP-Launcher");
        let mut cmd = Command::new(self.xlcore_install_directory.join(XIVLAUNCHER_BIN_FILENAME));
        // Write LD_PRELOAD as XL_PRELOAD, the launcher will use it if needed to pass this through to the game.
        if let Ok(ld_preload) = env::var("LD_PRELOAD")
            && !ld_preload.trim().is_empty()
        {
            cmd.env("XL_PRELOAD", ld_preload);
        }
        // Always remove LD_PRELOAD as Steam overlay will break the launcher text.
        cmd.env_remove("LD_PRELOAD");

        let exit_status = cmd.spawn()?.wait().await;

        if let Some(child) = discord_bridge.as_mut() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }

        let exit_status = exit_status?;
        match exit_status.success() {
            true => {
                info!("TruckersMP-Launcher exited cleanly: {exit_status}");
                Ok(())
            }
            false => {
                warn!("TruckersMP-Launcher did not exit with a successful status code: {exit_status}");
                Err(anyhow!(
                    "TruckersMP-Launcher did not exit with a successful status code: {exit_status}"
                ))
            }
        }
    }
}

async fn start_discord_bridge(install_dir: &Path) -> Option<Child> {
    let bridge_exe = install_dir.join(DISCORD_BRIDGE_FILENAME);
    if !fs::exists(&bridge_exe).ok()? {
        return None;
    }

    let runtime_dir = env::var("XDG_RUNTIME_DIR").ok()?;
    if !Path::new(&runtime_dir).join("discord-ipc-0").exists() {
        return None;
    }

    if std::process::Command::new("pgrep")
        .arg("-f")
        .arg("winediscordipcbridge.exe")
        .status()
        .ok()?
        .success()
    {
        return None;
    }

    let mut cmd = Command::new("umu-run");
    cmd.arg(&bridge_exe);
    cmd.env("WINEPREFIX", install_dir.join(DISCORD_BRIDGE_PREFIX_DIRNAME));
    cmd.env("GAMEID", env::var("GAMEID").unwrap_or_else(|_| "umu-0".to_string()));

    if let Ok(protonpath) = env::var("PROTONPATH")
        && !protonpath.trim().is_empty()
    {
        cmd.env("PROTONPATH", protonpath);
    }

    cmd.kill_on_drop(true);

    match cmd.spawn() {
        Ok(child) => {
            info!("Started Discord bridge");
            Some(child)
        }
        Err(err) => {
            warn!("Failed to start Discord bridge: {err:?}");
            None
        }
    }
}

/// Create/Overwrite an XLCore installation.
async fn install_or_update_xlcore<F: Fn(&str)>(
    release: ReleaseAssetInfo,
    aria_source: AriaSource,
    install_location: &Path,
    is_update: bool,
    progress_msg_cb: F,
) -> anyhow::Result<()> {
    // Download and create archive readers for required files.
    let mut xlcore_archive = {
        match is_update {
            true => {
                info!("Updating TruckersMP-Launcher from {}", release.download_url);
                progress_msg_cb(&format!("Updating TruckersMP-Launcher ({})", release.version));
            }
            false => {
                info!("Downloading TruckersMP-Launcher from {}", release.download_url);
                progress_msg_cb(&format!("Downloading TruckersMP-Launcher ({})", release.version));
            }
        }

        let response = reqwest::get(release.download_url).await?;
        let bytes = response.bytes().await?;
        Archive::new(GzDecoder::new(bytes.reader()))
    };
    let mut aria_archive = {
        match aria_source {
            AriaSource::Embedded => {
                info!("Using embedded aria2c tarball");
                Archive::new(GzDecoder::new(
                    Bytes::from_static(EMBEDDED_ARIA2C_TARBALL).reader(),
                ))
            }
            AriaSource::Url(url) => {
                info!("Downloading remote aria2c tarball from {url}");
                progress_msg_cb("Downloading aria2c");
                let response: reqwest::Response = reqwest::get(url).await?;
                Archive::new(GzDecoder::new(response.bytes().await?.reader()))
            }
            AriaSource::File(path) => {
                info!("Using local aria2c tarball at path: {path:?}");
                Archive::new(GzDecoder::new(Bytes::from(fs::read(path)?).reader()))
            }
        }
    };

    // Cleanup old install.
    let _ = fs::remove_dir_all(install_location);
    fs::create_dir_all(install_location)?;

    // Unpack XLCore
    info!("Unpacking TruckersMP-Launcher tarball");
    progress_msg_cb("Extracting TruckersMP-Launcher");
    xlcore_archive.unpack(install_location)?;
    drop(xlcore_archive);
    info!("Ensuring TruckersMP-Launcher tarball contained compatible files");
    progress_msg_cb("Validating TruckersMP-Launcher files");
    if !fs::exists(install_location.join(XIVLAUNCHER_BIN_FILENAME))? {
        bail!(
            "TruckersMP-Launcher tarball does not contain a file named '{}' and is incompatible with XLM.",
            XIVLAUNCHER_BIN_FILENAME
        )
    }
    info!("Successfully extracted and wrote TruckersMP-Launcher files");

    // Unpack Aria2c
    info!("Unpacking aria2c tarball");
    progress_msg_cb("Unpacking aria2c");
    aria_archive.unpack(install_location)?;
    drop(aria_archive);
    info!("Ensuring aria2c tarball contained compatible files");
    progress_msg_cb("Validating aria2c files");
    if !fs::exists(install_location.join(ARIA2C_BIN_FILENAME))? {
        bail!(
            "aria2c tarball does not contain a file named '{}' and is incompatible with XLM.",
            ARIA2C_BIN_FILENAME
        )
    }
    info!("Successfully extracted and wrote aria2c files");

    // Complete installation by writing version information.
    progress_msg_cb("Writing version data");
    let mut file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(install_location.join(XIVLAUNCHER_VERSIONDATA_LOCAL_FILENAME))?;
    file.write_all(release.version.as_bytes())?;
    info!("Wrote version data (version {})", release.version);
    progress_msg_cb("Finishing up");

    Ok(())
}

#[derive(Default, Clone, Debug)]
enum AriaSource {
    #[default]
    Embedded,
    Url(Url),
    File(PathBuf),
}

impl FromStr for AriaSource {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "embedded" => Ok(Self::Embedded),
            _ if s.starts_with("url:") => Ok(Self::Url(Url::parse(
                &s.chars().skip(4).collect::<String>(),
            )?)),
            _ if s.starts_with("file:") => {
                let s = s.chars().skip(5).collect::<String>();
                if !fs::exists(&s).context("exists check operation failed")? {
                    return Err(anyhow!("unable to find file at given path"));
                }
                Ok(Self::File(PathBuf::from(s)))
            }
            _ => Err(anyhow!("valid sources are 'embedded', 'url:' or 'file:'")),
        }
    }
}

impl Display for AriaSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            AriaSource::Embedded => write!(f, "embedded"),
            AriaSource::File(_) => write!(f, "file:"),
            AriaSource::Url(_) => write!(f, "url:"),
        }
    }
}

struct ReleaseAssetInfo {
    pub download_url: Url,
    pub version: String,
}

impl ReleaseAssetInfo {
    /// Obtain [`ReleaseAssetInfo`] from a web URL.
    pub async fn from_url(base_url: Url, release_asset: &str, version_asset: &str) -> Result<Self> {
        let (release_url, version_url) =
            (base_url.join(release_asset)?, base_url.join(version_asset)?);

        info!("release asset url: {release_url}");
        info!("release version url: {version_url}");

        let response = reqwest::get(version_url).await?;
        if !response.status().is_success() {
            bail!(
                "Did not receieve version information: {}",
                response.status(),
            );
        }

        Ok(Self {
            download_url: release_url,
            version: response.text().await?.trim().to_string(),
        })
    }
}
