//! Integration Tests for helm2oci
use std::process::Command;
use testcontainers::runners::SyncRunner;
use testcontainers_modules::cncf_distribution;

// Binary under test
const EXE: &str = env!("CARGO_BIN_EXE_helm2oci");

#[test]
fn test_e2e() {
    let out = test_temp_dir::TestTempDir::from_complete_item_path("it::test_e2e");
    let path = out.as_path_untracked().to_path_buf();

    // Create a helm chart
    let status = Command::new("helm")
        .arg("create")
        .arg("mychart")
        .current_dir(&path)
        .status()
        .unwrap();
    assert!(status.success());
    // Package our helm chart
    let status = Command::new("helm")
        .arg("package")
        .arg("mychart")
        .current_dir(&path)
        .status()
        .unwrap();
    assert!(status.success());

    // Convert the helm chart to an OCI layout
    let status = Command::new(EXE)
        .arg("--output")
        .arg("oci")
        .arg("mychart-0.1.0.tgz")
        .current_dir(&path)
        .status()
        .unwrap();
    assert!(status.success());

    let distribution_node = cncf_distribution::CncfDistribution::default().start().unwrap();
    let oci_ref = format!(
        "{}:{}/mychart:0.1.0",
        distribution_node.get_host().unwrap(),
        distribution_node.get_host_port_ipv4(5000).unwrap(),
    );
    let status = Command::new("oras")
        .arg("copy")
        .arg("--to-plain-http")
        .arg("--from-oci-layout")
        .arg("oci:0.1.0")
        .arg(&oci_ref)
        .current_dir(&path)
        .status()
        .unwrap();
    assert!(status.success());

    let helm_ref = format!(
        "oci://{}:{}/mychart",
        distribution_node.get_host().unwrap(),
        distribution_node.get_host_port_ipv4(5000).unwrap(),
    );
    // Check we can pull our helm chart
    let status = Command::new("helm")
        .arg("pull")
        .arg("--plain-http")
        .arg(helm_ref)
        .arg("--version")
        .arg("0.1.0")
        .current_dir(&path)
        .status()
        .unwrap();
    assert!(status.success());
}
