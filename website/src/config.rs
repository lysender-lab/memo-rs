use clap::Parser;
use serde::Deserialize;
use snafu::{ResultExt, ensure};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::error::{
    ConfigFileSnafu, ConfigParseSnafu, ConfigSnafu, ManifestParseSnafu, ManifestReadSnafu,
};

#[derive(Clone, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub ssl: bool,
    pub frontend_dir: PathBuf,
    pub captcha_site_key: String,
    pub captcha_api_key: String,
    pub api_url: String,
    pub jwt_secret: String,
    pub ga_tag_id: Option<String>,
}

#[derive(Clone, Deserialize)]
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
    pub gallery_js: String,
    pub upload_js: String,
    pub main_css: String,
    pub gallery_css: String,
}

impl Config {
    pub fn build(filename: &PathBuf) -> Result<Config> {
        let toml_string = fs::read_to_string(filename).context(ConfigFileSnafu)?;
        let config: AppConfig = toml::from_str(toml_string.as_str()).context(ConfigParseSnafu)?;

        // Validate config values
        ensure!(
            !config.jwt_secret.is_empty(),
            ConfigSnafu {
                msg: "JWT secret is required.".to_string()
            }
        );
        ensure!(
            !config.captcha_api_key.is_empty(),
            ConfigSnafu {
                msg: "Captcha API key is required.".to_string()
            }
        );
        ensure!(
            !config.captcha_site_key.is_empty(),
            ConfigSnafu {
                msg: "Captcha site key is required.".to_string()
            }
        );
        ensure!(
            !config.api_url.is_empty(),
            ConfigSnafu {
                msg: "API URL is required.".to_string()
            }
        );
        ensure!(
            config.port > 0,
            ConfigSnafu {
                msg: "Server port is required.".to_string()
            }
        );
        ensure!(
            config.frontend_dir.exists(),
            ConfigSnafu {
                msg: "Frontend directory does not exist.".to_string()
            }
        );

        let assets = AssetManifest::build(&config.frontend_dir)?;

        Ok(Config {
            port: config.port,
            ssl: config.ssl,
            frontend_dir: config.frontend_dir,
            captcha_site_key: config.captcha_site_key,
            captcha_api_key: config.captcha_api_key,
            api_url: config.api_url,
            jwt_secret: config.jwt_secret,
            ga_tag_id: config.ga_tag_id,
            assets,
        })
    }
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

        let gallery_css = config_map
            .get("bundles/gallery.css")
            .expect("gallery.css bundle is required");

        let main_js = config_map
            .get("bundles/main.js")
            .expect("main.js bundle is required");

        let upload_js = config_map
            .get("bundles/upload.js")
            .expect("upload.js bundle is required");

        let gallery_js = config_map
            .get("bundles/gallery.js")
            .expect("gallery.js bundle is required");

        Ok(AssetManifest {
            main_js: format!("/assets/bundles/{}", main_js.file),
            gallery_js: format!("/assets/bundles/{}", gallery_js.file),
            upload_js: format!("/assets/bundles/{}", upload_js.file),
            main_css: format!("/assets/bundles/{}", main_css.file),
            gallery_css: format!("/assets/bundles/{}", gallery_css.file),
        })
    }
}

/// memo-webite Make memories
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, value_name = "config.toml")]
    pub config: PathBuf,
}
