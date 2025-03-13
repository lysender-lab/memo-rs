use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;

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

#[derive(Clone, Deserialize)]
pub struct AssetManifest {
    pub main_js: String,
    pub vendor_js: String,
    pub gallery_js: String,
    pub upload_js: String,
    pub main_css: String,
    pub gallery_css: String,
}

#[derive(Deserialize)]
struct BundleConfig {
    suffix: String,
}

impl Config {
    pub fn build(filename: &PathBuf) -> Result<Config> {
        let toml_string = match fs::read_to_string(filename) {
            Ok(str) => str,
            Err(e) => {
                return Err(format!("Error reading config file: {}", e).into());
            }
        };

        let config: AppConfig = match toml::from_str(toml_string.as_str()) {
            Ok(value) => value,
            Err(e) => {
                return Err(format!("Error parsing config file: {}", e).into());
            }
        };

        // Validate config values
        if config.jwt_secret.len() == 0 {
            return Err("JWT secret is required.".into());
        }
        if config.captcha_api_key.len() == 0 {
            return Err("Captcha API key is required.".into());
        }
        if config.captcha_site_key.len() == 0 {
            return Err("Captcha site key is required.".into());
        }
        if config.api_url.len() == 0 {
            return Err("API URL is required.".into());
        }
        if config.port == 0 {
            return Err("Server port is required.".into());
        }

        if !config.frontend_dir.exists() {
            return Err("Frontend directory does not exist.".into());
        }

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
        let filename = Path::new(frontend_dir).join("bundles.json");
        let Ok(contents) = fs::read_to_string(filename) else {
            return Err("Failed to read bundles.json".into());
        };
        let bundle_res = serde_json::from_str::<BundleConfig>(contents.as_str());
        let Ok(config) = bundle_res else {
            return Err("Failed to parse bundles.json".into());
        };

        Ok(AssetManifest {
            main_js: format!("/assets/bundles/js/main-{}.js", config.suffix),
            vendor_js: format!("/assets/bundles/js/vendor-{}.js", config.suffix),
            gallery_js: format!("/assets/bundles/js/gallery-{}.js", config.suffix),
            upload_js: format!("/assets/bundles/js/upload-{}.js", config.suffix),
            main_css: format!("/assets/bundles/css/main-{}.css", config.suffix),
            gallery_css: format!("/assets/bundles/css/gallery-{}.css", config.suffix),
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
