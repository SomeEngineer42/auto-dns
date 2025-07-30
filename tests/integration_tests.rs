use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_write_config_flag() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("test_config.toml");

    // Prepare input for the interactive config creation
    let input = "us-west-2\ntest_access_key\ntest_secret\n2\nZ123456789\nhome.example.com\n\nZ987654321\noffice.example.com\n600\n";

    // Run the binary with --write-config flag
    let mut child = Command::new("cargo")
        .args(&["run", "--", "--write-config", config_path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start process");

    // Send input to the process
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(input.as_bytes()).expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");
    }

    // Wait for the process to complete
    let output = child.wait_with_output().expect("Failed to wait for process");

    // Debug output if the test fails
    if !output.status.success() {
        println!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        println!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Check that the process succeeded
    assert!(output.status.success(),
        "Process failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr));

    // Verify the config file was created
    assert!(config_path.exists(), "Config file was not created");

    // Read and verify the generated config
    let config_content = fs::read_to_string(&config_path)
        .await
        .expect("Failed to read config file");

    println!("Generated config:\n{}", config_content);

    // Parse the TOML to ensure it's valid
    let parsed_config: toml::Table = toml::from_str(&config_content)
        .expect("Generated config is not valid TOML");

    // Verify the AWS section
    let aws_section = parsed_config.get("aws")
        .and_then(|v| v.as_table())
        .expect("AWS section should exist");

    assert_eq!(aws_section.get("region")
        .and_then(|v| v.as_str())
        .expect("Region should be a string"), "us-west-2");

    assert_eq!(aws_section.get("access_key_id")
        .and_then(|v| v.as_str())
        .expect("Access key should be a string"), "test_access_key");

    assert_eq!(aws_section.get("secret_access_key")
        .and_then(|v| v.as_str())
        .expect("Secret key should be a string"), "test_secret");

    // Verify the records section
    let records = parsed_config.get("records")
        .and_then(|v| v.as_array())
        .expect("Records should be an array");

    assert_eq!(records.len(), 2);

    // Verify first record
    let first_record = records[0].as_table().expect("Record should be a table");
    assert_eq!(first_record.get("hosted_zone_id")
        .and_then(|v| v.as_str())
        .expect("Hosted zone ID should be a string"), "Z123456789");

    assert_eq!(first_record.get("name")
        .and_then(|v| v.as_str())
        .expect("Name should be a string"), "home.example.com");

    assert_eq!(first_record.get("ttl")
        .and_then(|v| v.as_integer())
        .expect("TTL should be an integer"), 300);

    // Verify second record
    let second_record = records[1].as_table().expect("Record should be a table");
    assert_eq!(second_record.get("hosted_zone_id")
        .and_then(|v| v.as_str())
        .expect("Hosted zone ID should be a string"), "Z987654321");

    assert_eq!(second_record.get("name")
        .and_then(|v| v.as_str())
        .expect("Name should be a string"), "office.example.com");

    assert_eq!(second_record.get("ttl")
        .and_then(|v| v.as_integer())
        .expect("TTL should be an integer"), 600);
}

#[tokio::test]
async fn test_write_config_with_other_flags_fails() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("test_config.toml");

    // Test that --write-config with --once fails
    let output = Command::new("cargo")
        .args(&["run", "--", "--write-config", config_path.to_str().unwrap(), "--once"])
        .output()
        .expect("Failed to run command");

    assert!(!output.status.success(), "Command should fail when using --write-config with --once");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--write-config cannot be used with other flags"),
        "Should show error message about conflicting flags. Actual stderr: {}", stderr);

    // Test that --write-config with --no-aws fails
    let output = Command::new("cargo")
        .args(&["run", "--", "--write-config", config_path.to_str().unwrap(), "--no-aws"])
        .output()
        .expect("Failed to run command");

    assert!(!output.status.success(), "Command should fail when using --write-config with --no-aws");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--write-config cannot be used with other flags"),
        "Should show error message about conflicting flags. Actual stderr: {}", stderr);
}

#[tokio::test]
async fn test_write_config_minimal_input() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("minimal_config.toml");

    // Test with minimal input (empty AWS credentials, one record with default TTL)
    let input = "eu-central-1\n\n\n1\nZ111111111\napi.test.com\n\n";

    let mut child = Command::new("cargo")
        .args(&["run", "--", "--write-config", config_path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start process");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(input.as_bytes()).expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");
    }

    let output = child.wait_with_output().expect("Failed to wait for process");

    if !output.status.success() {
        println!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        println!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    }

    assert!(output.status.success(),
        "Process failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr));

    let config_content = fs::read_to_string(&config_path)
        .await
        .expect("Failed to read config file");

    let parsed_config: toml::Table = toml::from_str(&config_content)
        .expect("Generated config is not valid TOML");

    // Verify minimal config structure
    let aws_section = parsed_config.get("aws")
        .and_then(|v| v.as_table())
        .expect("AWS section should exist");

    assert_eq!(aws_section.get("region")
        .and_then(|v| v.as_str())
        .expect("Region should be a string"), "eu-central-1");

    // With empty credentials, they shouldn't be included in the TOML
    assert!(aws_section.get("access_key_id").is_none());
    assert!(aws_section.get("secret_access_key").is_none());

    let records = parsed_config.get("records")
        .and_then(|v| v.as_array())
        .expect("Records should be an array");

    assert_eq!(records.len(), 1);

    let record = records[0].as_table().expect("Record should be a table");
    assert_eq!(record.get("hosted_zone_id")
        .and_then(|v| v.as_str())
        .expect("Hosted zone ID should be a string"), "Z111111111");

    assert_eq!(record.get("name")
        .and_then(|v| v.as_str())
        .expect("Name should be a string"), "api.test.com");

    assert_eq!(record.get("ttl")
        .and_then(|v| v.as_integer())
        .expect("TTL should be an integer"), 300);
}

#[tokio::test]
async fn test_no_aws_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("test_config.toml");

    // Create a test configuration file
    let config_content = r#"[aws]
region = "us-east-1"
access_key_id = "test_key"
secret_access_key = "test_secret"

[[records]]
hosted_zone_id = "Z123456789ABC"
name = "test.example.com"
ttl = 300
"#;

    fs::write(&config_path, config_content).await.expect("Failed to write config file");

    // Test --no-aws flag with --once
    let output = Command::new("cargo")
        .args(&["run", "--", "--config", config_path.to_str().unwrap(), "--once", "--no-aws"])
        .output()
        .expect("Failed to run command");

    // Check that the process succeeded
    assert!(output.status.success(),
        "Process failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify that dry-run mode is activated
    assert!(stdout.contains("Running in dry-run mode (--no-aws)"),
        "Should indicate dry-run mode. Actual stdout: {}", stdout);

    // Verify that mock DNS operations are logged
    assert!(stdout.contains("[DRY RUN]"),
        "Should contain dry-run log messages. Actual stdout: {}", stdout);

    // Verify that it shows what would be done
    assert!(stdout.contains("Would update DNS record"),
        "Should show what DNS update would be performed. Actual stdout: {}", stdout);

    // Verify that AWS API calls are mentioned as simulated
    assert!(stdout.contains("AWS Route53 API call would be made"),
        "Should mention simulated AWS API calls. Actual stdout: {}", stdout);
}
