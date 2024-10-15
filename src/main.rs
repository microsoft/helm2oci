// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
use anyhow::{anyhow, Context, Result};
use argh::FromArgs;
use flate2::read::GzDecoder;
use ocidir::oci_spec::image::{ImageManifestBuilder, MediaType};
use ocidir::{cap_std::fs::Dir, OciDir};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use tar::Archive;

const HELM_CHART_YAML_FILE: &str = "Chart.yaml";
const HELM_CONFIG_MEDIA_TYPE: &str = "application/vnd.cncf.helm.config.v1+json";
const HELM_CONTENT_MEDIA_TYPE: &str = "application/vnd.cncf.helm.chart.content.v1.tar+gzip";

#[derive(FromArgs, Debug)]
/// Convert Helm chart archive to OCI layout
pub struct Cli {
    /// path to output directory.
    /// The directory is created if it does not exist.
    /// Defaults to the chart name.
    #[argh(option)]
    output: Option<PathBuf>,
    /// path to Helm chart archive
    #[argh(positional)]
    chart: PathBuf,
}

pub fn main() -> Result<()> {
    let args: Cli = argh::from_env();
    // extract the chart.yaml file from the helm chart
    let helm_manifest = get_manifest_from_archive(&args.chart)?;
    let name = helm_manifest["name"].as_str().ok_or_else(|| {
        anyhow!(
            "Chart.yaml doesn't contain a name field. manifest: {}",
            helm_manifest
        )
    })?;
    let version = helm_manifest["version"].as_str().ok_or_else(|| {
        anyhow!(
            "Chart.yaml doesn't contain a name field. manifest: {}",
            helm_manifest
        )
    })?;

    let output = args.output.unwrap_or_else(|| PathBuf::from(&name));
    fs::create_dir_all(&output)
        .context(format!("Failed to create OCI image directory `{}`", &name))?;
    let dir = Dir::open_ambient_dir(&output, ocidir::cap_std::ambient_authority())
        .context("Failed to open image directory")?;
    let oci = OciDir::ensure(&dir)?;

    // Write chart contents to a layer
    let mut w = oci.create_blob().context("Failed to create blob")?;
    let mut f = File::open(&args.chart).context(format!(
        "Failed to open Chart archive {}",
        args.chart.display()
    ))?;
    copy(&mut f, &mut w).context("Failed to write chart layer")?;

    let layer_descriptor = w
        .complete()
        .context("Failed to finish Chart blob")?
        .descriptor()
        .media_type(MediaType::Other(HELM_CONTENT_MEDIA_TYPE.to_string()))
        .build()?;

    // Write chart metadata to a blob
    let config_descriptor = oci
        .write_json_blob(
            &helm_manifest,
            MediaType::Other(HELM_CONFIG_MEDIA_TYPE.to_string()),
        )?
        .build()?;

    // Write image manifest
    let manifest = ImageManifestBuilder::default()
        .schema_version(2u32)
        .media_type(MediaType::ImageManifest)
        .layers(vec![layer_descriptor])
        .config(config_descriptor)
        .build()?;

    oci.insert_manifest(
        manifest,
        Some(version),
        ocidir::oci_spec::image::Platform::default(),
    )?;
    Ok(())
}

/// read Chart.yaml from archive
fn get_manifest_from_archive(tgz_path: impl AsRef<Path>) -> Result<serde_json::Value> {
    let tgz_path = tgz_path.as_ref();
    let tgz = File::open(tgz_path)
        .context(format!("Failed to open helm chart {}", tgz_path.display()))?;
    let tar = GzDecoder::new(tgz);
    let mut archive = Archive::new(tar);
    // Find the Chart.yaml entry in the archive
    let mut chart_yaml_entry = archive
        .entries()?
        .find(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|entry| entry.path().ok())
                // We're looking for <chart name>/Chart.yaml file, but don't know the name of the chart
                .map(|p| {
                    p.components().collect::<Vec<_>>().len() == 2
                        && p.file_name() == Some(OsStr::new(HELM_CHART_YAML_FILE))
                })
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow!("Chart.yaml not found in the helm chart"))??;
    let mut contents = Vec::new();
    std::io::copy(&mut chart_yaml_entry, &mut contents)?;
    Ok(serde_yml::from_slice(&contents)?)
}
