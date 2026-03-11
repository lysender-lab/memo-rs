use clap::Parser;
use serde::Deserialize;
use snafu::{ResultExt, ensure};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::error::{ConfigSnafu, ManifestParseSnafu, ManifestReadSnafu};

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub ssl: bool,
    pub frontend_dir: PathBuf,
    pub captcha_site_key: String,
    pub captcha_api_key: String,
    pub api_url: String,
    pub jwt_secret: String,
    pub ga_tag_id: Option<String>,
    pub assets: AssetManifest,
}

#[derive(Deserialize)]
struct BundleEntry {
    pub file: String,
}

type BundleConfigMap = HashMap<String, BundleEntry>;

#[derive(Clone, Deserialize)]
pub struct AssetManifest {
    pub main_js: String,
    pub main_css: String,
}

impl Config {
    pub fn build_from_env() -> Result<Config> {
        let port = required_env_parse::<u16>("PORT")?;
        let ssl = required_env_parse::<bool>("SSL")?;
        let frontend_dir = PathBuf::from(required_env("FRONTEND_DIR")?);
        let captcha_site_key = required_env("CAPTCHA_SITE_KEY")?;
        let captcha_api_key = required_env("CAPTCHA_API_KEY")?;
        let api_url = required_env("API_URL")?;
        let jwt_secret = required_env("JWT_SECRET")?;
        let ga_tag_id = optional_env("GA_TAG_ID");

        // Validate config values
        ensure!(
            !jwt_secret.is_empty(),
            ConfigSnafu {
                msg: "JWT secret is required.".to_string()
            }
        );
        ensure!(
            !captcha_api_key.is_empty(),
            ConfigSnafu {
                msg: "Captcha API key is required.".to_string()
            }
        );
        ensure!(
            !captcha_site_key.is_empty(),
            ConfigSnafu {
                msg: "Captcha site key is required.".to_string()
            }
        );
        ensure!(
            !api_url.is_empty(),
            ConfigSnafu {
                msg: "API URL is required.".to_string()
            }
        );
        ensure!(
            port > 0,
            ConfigSnafu {
                msg: "Server port is required.".to_string()
            }
        );
        ensure!(
            frontend_dir.exists(),
            ConfigSnafu {
                msg: "Frontend directory does not exist.".to_string()
            }
        );

        let assets = AssetManifest::build(&frontend_dir)?;

        Ok(Config {
            port,
            ssl,
            frontend_dir,
            captcha_site_key,
            captcha_api_key,
            api_url,
            jwt_secret,
            ga_tag_id,
            assets,
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    match env::var(name) {
        Ok(val) => Ok(val),
        Err(_) => ConfigSnafu {
            msg: format!("{} is required.", name),
        }
        .fail(),
    }
}

fn optional_env(name: &str) -> Option<String> {
    match env::var(name) {
        Ok(val) if !val.trim().is_empty() => Some(val),
        _ => None,
    }
}

fn required_env_parse<T>(name: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    let value = required_env(name)?;
    value.parse::<T>().map_err(|_| crate::Error::Config {
        msg: format!("{} is invalid.", name),
    })
}

impl AssetManifest {
    pub fn build(frontend_dir: &PathBuf) -> Result<Self> {
        let filename = Path::new(frontend_dir).join("public/assets/bundles/.vite/manifest.json");
        let contents = fs::read_to_string(filename).context(ManifestReadSnafu)?;
        let config_map = serde_json::from_str::<BundleConfigMap>(contents.as_str())
            .context(ManifestParseSnafu)?;

        let main_css = config_map
            .get("bundles/main.css")
            .expect("main.css bundle is required");

        let main_js = config_map
            .get("bundles/main.js")
            .expect("main.js bundle is required");

        Ok(AssetManifest {
            main_js: format!("/assets/bundles/{}", main_js.file),
            main_css: format!("/assets/bundles/{}", main_css.file),
        })
    }
}

/// memo-webite Make memories
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {}
