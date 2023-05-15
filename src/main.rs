mod error;
mod extensions;
mod helm;

use crate::error::AdelieError;
use crate::extensions::HyperToString;
use crate::helm::Chart;
use color_eyre::eyre::Result;
use color_eyre::Report;
use futures::{stream, StreamExt};
use octocrab::models::repos::CommitAuthor;
use octocrab::params::repos::Reference;
use toml_edit::{value, Document};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let github = octocrab::OctocrabBuilder::default()
        .personal_token(std::env::var("GITHUB_TOKEN")?)
        .build()?;
    let org = "ocf";
    let repo = "kubernetes";
    let version_file = "apps/versions.toml";

    let versions = github
        .repos(org, repo)
        .raw_file(Reference::Branch("main".into()), "apps/versions.toml")
        .await?
        .body_mut()
        .hyper_to_string()
        .await?
        .parse::<Document>()?;

    let update_sha = github
        .repos(org, repo)
        .get_content()
        .path(version_file)
        .r#ref("main")
        .send()
        .await?
        .items
        .iter()
        .filter(|x| x.name.eq("versions.toml"))
        .next()
        .ok_or(AdelieError::Misc("no file".into()))?
        .sha
        .to_string();

    let out_versions = versions.clone();

    let i: Vec<_> = stream::iter(versions.iter())
        // Parse the items into a useful format...
        .then(|(name, value)| async move {
            let chart = match value.get("chart") {
                None => name,
                Some(v) => v.as_str().ok_or(AdelieError::Misc("".into()))?,
            };
            let repo = value
                .get("helm")
                .ok_or(AdelieError::Misc("".into()))?
                .as_str()
                .ok_or(AdelieError::Misc("".into()))?;
            let version = value
                .get("version")
                .ok_or(AdelieError::Misc("".into()))?
                .as_str()
                .ok_or(AdelieError::Misc("".into()))?;
            Ok::<_, Report>((chart, repo, version, name))
        })
        // Ignore any errors...
        .filter_map(|x| async move { x.ok() })
        .map(|(chart, repo, version, name)| async move {
            let mut helmchart = Chart {
                name: chart.into(),
                repo: repo.into(),
                version: Some(version.into()),
                app_version: None,
            };
            helmchart.update_version().await?;
            let new_version = helmchart.version.unwrap();
            let new_version = if new_version.contains("v") {
                &new_version[1..]
            } else {
                &new_version
            }
            .to_string();
            if !version.eq(&new_version) {
                println!("Update found: {chart} from {version} -> {}", &new_version);
                return Ok::<_, Report>((new_version, name.to_string()));
            }
            Err::<_, Report>(AdelieError::Misc("No new version found.".into()).into())
        })
        // Ignore any errors...
        .filter_map(|x| async move { x.await.ok() })
        .map(|(new_version, name)| {
            let mut out = out_versions.clone();
            out[&name]["version"] = value(new_version.clone());
            let out = out.to_string();
            (new_version, name, out)
        })
        .collect::<Vec<_>>()
        .await;

    let main_sha = github
        .repos(org, repo)
        .get_ref(&Reference::Branch("main".to_string()))
        .await?;
    let main_sha = match main_sha.object {
        octocrab::models::repos::Object::Commit { sha, url: _ } => sha,
        octocrab::models::repos::Object::Tag { sha, url: _ } => sha,
        _ => todo!(),
    };

    for (version, name, contents) in i {
        // Make GitHub PR...
        let branch_name = format!("u-{}-{}", &name, &version);
        let pr_title = format!("feat: update {} -> v{}", &name, &version);
        github
            .repos(org, repo)
            .create_ref(&Reference::Branch(branch_name.clone()), &main_sha)
            .await?;
        github
            .repos(org, repo)
            .update_file(
                version_file,
                &pr_title,
                &contents,
                &update_sha,
            )
            .branch(&branch_name)
            .commiter(CommitAuthor {
                name: "ocfbot".to_string(),
                email: "ocfbot@ocf.berkeley.edu".to_string(),
            })
            .author(CommitAuthor {
                name: "ocfbot".to_string(),
                email: "ocfbot@ocf.berkeley.edu".to_string(),
            })
            .send()
            .await?;
        github
            .pulls(org, repo)
            .create(&pr_title, &branch_name, "main")
            .body("Please be sure to test this change before merging!")
            .send()
            .await?;
    }

    Ok(())
}
