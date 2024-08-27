//! Copyright (c) Microsoft Corporation. All rights reserved.
//! Highly Confidential Material
use anyhow::{bail, Context, Result};
use clap::Parser;
use flate2::read::GzDecoder;
use oci_spec::image::{
    Descriptor, DescriptorBuilder, ImageIndex, ImageManifestBuilder, MediaType, OciLayoutBuilder,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use tar::Archive;
use tempfile::NamedTempFile;
use tempfile::TempDir;

mod sha256_writer;
pub mod write;

use crate::sha256_writer::Sha256Writer;

const OCI_LAYOUT_FILE: &str = "oci-layout";
// OCI layout version: https://github.com/opencontainers/image-spec/blob/main/image-layout.md#oci-layout-file
// Fixed at 1.0.0 until changes to the layout format are made, when changes are made the new version will be
// taken from the OCI spec version that makes the changes.
const OCI_LAYOUT_VERSION: &str = "1.0.0";

const HELM_CHART_YAML_FILE: &str = "Chart.yaml";

const OCI_DIR: &str = "oci_dir";

const HELM_CONFIG_MEDIA_TYPE: &str = "application/vnd.cncf.helm.config.v1+json";
const HELM_CONTENT_MEDIA_TYPE: &str = "application/vnd.cncf.helm.chart.content.v1.tar+gzip";

#[derive(Parser)]
#[command(version, about = "Helm chart to OCI layout converter", long_about = None)]
pub struct Cli {
    /// The name of the helm chart to convert
    #[arg(long)]
    helm_chart_name: String,
    /// The tag of the helm chart to convert
    #[arg(long)]
    tag: String,
    /// The output directory to store the OCI layout
    #[arg(long)]
    output_dir: Option<PathBuf>,
    /// The path to the directory holding the helm chart
    #[arg(long)]
    path_to_helm: Option<PathBuf>,
}

pub fn run(cli: Cli) -> Result<()> {
    let helm_chart_tgz = format!("{}-{}.tgz", cli.helm_chart_name, cli.tag);

    let path = if let Some(path_to_helm) = cli.path_to_helm {
        path_to_helm.join(helm_chart_tgz)
    } else {
        env::current_dir().unwrap().join(helm_chart_tgz)
    };

    if !path.exists() {
        bail!("File not found: {}", path.display());
    }

    let oci_dir = if let Some(output_dir) = cli.output_dir {
        output_dir
    } else {
        PathBuf::from(OCI_DIR)
    };

    write::ok("Creating", "oci layout directory")?;
    init_oci_layout_dir(&oci_dir).expect("Error initialising OCI layout directory");
    let chart_json = get_chart_from_tgz(&path, cli.helm_chart_name)?;

    // Write Chart.yaml file as a blob in the OCI directory
    write::ok("Writing", "config blob")?;
    let config = write_blob(
        Some(&chart_json),
        None,
        MediaType::from(HELM_CONFIG_MEDIA_TYPE),
        &oci_dir,
    )?;
    // Write the helm chart tgz as a blob in the OCI directory
    write::ok("Writing", "image layer blob")?;
    let layer = write_blob::<serde_json::Value>(None, Some(&path), MediaType::from(HELM_CONTENT_MEDIA_TYPE), &oci_dir)?;

    write::ok("Writing", "image manifest")?;
    let manifest = ImageManifestBuilder::default()
        .schema_version(2u32)
        .media_type(MediaType::ImageManifest)
        .layers(vec![layer])
        .config(config)
        .build()?;
    let mut manifest_descriptor = write_blob(Some(&manifest), None, MediaType::ImageManifest, &oci_dir)?;
    let mut annotations = HashMap::new();
    annotations.insert(
        "org.opencontainers.image.ref.name".to_string(),
        cli.tag.to_string(),
    );
    manifest_descriptor.set_annotations(Some(annotations));

    // Add the manifest descriptor to the OCI image index
    write::ok("Adding", "manifest to OCI image index")?;
    let index_path = oci_dir.join("index.json");
    let mut index: ImageIndex = serde_json::from_str(
        &fs::read_to_string(&index_path)
            .context(format!("Failed to read `{}`", index_path.display()))?,
    )?;

    index.set_manifests(vec![manifest_descriptor]);

    let index_file = std::fs::File::create(&index_path).context(format!(
        "Failed to create index.json file `{}`",
        index_path.display()
    ))?;
    serde_json::to_writer(index_file, &index).context(format!(
        "Failed to write to index.json file `{}`",
        index_path.display()
    ))?;
    Ok(())
}

// unpack tar ball to get the Chart.yaml file
pub fn get_chart_from_tgz(
    tgz_path: impl AsRef<Path>,
    helm_chart_name: String,
) -> Result<serde_json::Value> {
    let tmp_rpm_dir = TempDir::new()?;
    let tmp_path = tmp_rpm_dir.path();
    let tar_gz = File::open(tgz_path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    if let Err(e) = archive.unpack(tmp_path) {
        bail!("Failed to unpack tarball: {}", e);
    }

    let chart_path = tmp_path.join(helm_chart_name).join(HELM_CHART_YAML_FILE);
    let chart_file = File::open(chart_path)?;
    let chart_json: serde_json::Value = serde_yml::from_reader(chart_file)?;

    Ok(chart_json)
}

/// Direct copy from rpmoci plus edit in the writing to the writer
/// Write a json object with the specified media type to the specified
/// OCI layout directory
pub(crate) fn write_blob<T>(
    value: Option<&T>,
    path_to_tgz: Option<&Path>,
    media_type: MediaType,
    layout_path: impl AsRef<Path>,
) -> Result<Descriptor>
where
    T: ?Sized + Serialize,
{
    let mut writer = Sha256Writer::new(NamedTempFile::new()?);
    if let Some(value) = value {
        serde_json::to_writer(&mut writer, value)
            .context("Failed to write to blob to temporary file")?;
    } else if let Some(path_to_tgz) = path_to_tgz {
        let data = fs::read(path_to_tgz)?;
        if let Err(e) = writer.write(&data) {
            bail!("Failed to write to blob to temporary file: {}", e);
        }
    } else {
        bail!("No value or path to tgz provided");
    }
    writer.flush()?;
    let (blob_sha, tmp_file) = writer.finish();
    let blob_path = layout_path.as_ref().join("blobs/sha256").join(&blob_sha);

    let (blob, tmp_path) = tmp_file.keep()?;
    let size: i64 = blob.metadata()?.len().try_into()?;
    // May fail if tempfile on different filesystem
    if fs::rename(&tmp_path, &blob_path).is_err() {
        fs::copy(&tmp_path, &blob_path)
            .context(format!("Failed to write blob `{}`", blob_path.display()))?;
    }

    Ok(DescriptorBuilder::default()
        .digest(format!("sha256:{}", blob_sha))
        .media_type(media_type)
        .size(size)
        .build()?)
}

/// From rpmoci and heavily edited
/// Initialize an [OCI image directory](https://github.com/opencontainers/image-spec/blob/main/image-layout.md) if required
///
/// If the directory doesn't exist, it will be created.
/// Returns an error if the directory exists already and is not empty.
pub fn init_oci_layout_dir(oci_dir: impl AsRef<Path>) -> Result<(), anyhow::Error> {
    if oci_dir.as_ref().exists() {
        if fs::read_dir(oci_dir.as_ref())
            .map(|dir| dir.count() == 0)
            .unwrap_or(false)
        {
            init_dir(oci_dir.as_ref())?;
        } else {
            bail!(
                "Directory `{}` already exists and is not empty",
                oci_dir.as_ref().display()
            )
        }
    } else {
        fs::create_dir_all(oci_dir.as_ref()).context(format!(
            "Failed to create OCI layout directory `{}`",
            oci_dir.as_ref().display()
        ))?;
        init_dir(oci_dir.as_ref())?;
    }
    Ok(())
}

/// Direct copy from rpmoci
/// Create blobs/sha256, index.json and oci-layout file in a directory
fn init_dir(layout: impl AsRef<Path>) -> Result<(), anyhow::Error> {
    // Create blobs directory
    let blobs_dir = layout.as_ref().join("blobs").join("sha256");
    fs::create_dir_all(&blobs_dir).context(format!(
        "Failed to create blobs/sha256 directory `{}`",
        blobs_dir.display()
    ))?;

    // create oci-layout file
    let oci_layout = OciLayoutBuilder::default()
        .image_layout_version(OCI_LAYOUT_VERSION)
        .build()?;
    let oci_layout_path = layout.as_ref().join(OCI_LAYOUT_FILE);
    oci_layout.to_file(&oci_layout_path).context(format!(
        "Failed to write to oci-layout file `{}`",
        oci_layout_path.display()
    ))?;

    // create image index
    let index = oci_spec::image::ImageIndexBuilder::default()
        .manifests(Vec::new())
        .schema_version(2u32)
        .build()?;
    let index_path = layout.as_ref().join("index.json");
    let index_file = std::fs::File::create(&index_path).context(format!(
        "Failed to create index.json file `{}`",
        index_path.display()
    ))?;
    serde_json::to_writer(index_file, &index).context(format!(
        "Failed to write to index.json file `{}`",
        index_path.display()
    ))?;

    Ok(())
}
