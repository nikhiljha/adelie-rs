use std::collections::HashMap;
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use url::Url;
use crate::error::AdelieError;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HelmIndex {
    pub api_version: String,
    pub entries: HashMap<String, Vec<HelmIndexVersion>>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HelmIndexVersion {
    pub version: String,
    pub app_version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    pub name: String,
    pub repo: String,
    pub version: Option<String>,
    pub app_version: Option<String>,
}

impl Chart {
    pub async fn update_version(&mut self) -> Result<()> {
        let mut url = Url::parse(&*format!("{}/", &self.repo))?.join("index.yaml")?;
        println!("{:}", url);
        let index: HelmIndex = serde_yaml::from_str(
            &reqwest::get(url)
                .await?
                .text()
                .await?
        )?;
        let entry = index.entries
            .get(&self.name)
            .ok_or(AdelieError::Misc("".into()))?
            .iter()
            // <https://github.com/helm/helm/blob/2398830f183b6d569224ae693ae9215fed5d1372/cmd/helm/search_repo.go#L138>
            .filter(|x| { !x.version.contains("-") })
            // Assume the newest version is on top...
            // I was considering parsing SemVer but... why do more work when less does
            // the trick? Feel free to PR if this breaks something for you.
            .next()
            .ok_or(AdelieError::Misc("".into()))?;

        self.version = Some(entry.version.clone());
        self.app_version = entry.app_version.clone();

        Ok(())
    }
}
