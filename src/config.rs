use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::{env, fs};

use crate::Result;

pub const PORT: &str = "PORT";
pub const SSL: &str = "SSL";
pub const FRONTEND_DIR: &str = "FRONTEND_DIR";
pub const CAPTCHA_SITE_KEY: &str = "CAPTCHA_SITE_KEY";
pub const CAPTCHA_API_KEY: &str = "CAPTCHA_API_KEY";
pub const API_URL: &str = "API_URL";
pub const JWT_SECRET: &str = "JWT_SECRET";
pub const GA_TAG_ID: &str = "GA_TAG_ID";

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
    pub fn build() -> Result<Config> {
        dotenv().ok();

        let env_port = env::var(PORT).expect("PORT is not set");
        let port: u16 = env_port.parse().expect("PORT is not a valid number");
        let env_ssl = env::var(SSL).expect("SSL is not set");
        let ssl = env_ssl.as_str() == "1";
        let env_frontend_dir = env::var(FRONTEND_DIR).expect("FRONTEND_DIR is not set");
        let frontend_dir = PathBuf::from(env_frontend_dir);
        let captcha_site_key: String =
            env::var(CAPTCHA_SITE_KEY).expect("CAPTCHA_SITE_KEY is not set");
        let captcha_api_key: String =
            env::var(CAPTCHA_API_KEY).expect("CAPTCHA_API_KEY is not set");
        let api_url: String = env::var(API_URL).expect("API_URL is not set");
        let jwt_secret: String = env::var(JWT_SECRET).expect("JWT_SECRET is not set");
        let ga_tag_id: Option<String> = env::var(GA_TAG_ID).ok();

        if !frontend_dir.exists() {
            return Err("Frontend dir does not exists.".into());
        }

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

/// memo-rs A photo gallery app
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Runs the web server
    Server,
}
