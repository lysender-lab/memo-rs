use crate::{config::AssetManifest, run::AppState};

use super::Pref;
use memo::actor::Actor;

#[derive(Clone)]
pub struct TemplateData {
    pub theme: String,
    pub title: String,
    pub assets: AssetManifest,
    pub styles: Vec<String>,
    pub scripts: Vec<String>,
    pub async_scripts: Vec<String>,
    pub script_vars: Vec<String>,
    pub ga_tag_id: Option<String>,
    pub actor: Option<Actor>,
}

impl TemplateData {
    pub fn new(state: &AppState, actor: Option<Actor>, pref: &Pref) -> TemplateData {
        let config = state.config.clone();
        let assets = config.assets.clone();

        TemplateData {
            theme: pref.theme.clone(),
            title: String::from(""),
            assets,
            styles: Vec::new(),
            scripts: Vec::new(),
            async_scripts: Vec::new(),
            script_vars: Vec::new(),
            ga_tag_id: config.ga_tag_id.clone(),
            actor,
        }
    }
}
