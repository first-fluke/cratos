//! Tests for sandbox module

use super::*;
use std::time::Duration;

#[test]
fn test_sandbox_policy() {
    assert!(SandboxPolicy::Strict.should_sandbox("low"));
    assert!(SandboxPolicy::Strict.should_sandbox("high"));

    assert!(!SandboxPolicy::Moderate.should_sandbox("low"));
    assert!(SandboxPolicy::Moderate.should_sandbox("medium"));
    assert!(SandboxPolicy::Moderate.should_sandbox("high"));

    assert!(!SandboxPolicy::Disabled.should_sandbox("high"));
}

#[test]
fn test_network_mode() {
    assert_eq!(NetworkMode::None.as_docker_arg(), "none");
    assert_eq!(NetworkMode::Bridge.as_docker_arg(), "bridge");
    assert_eq!(NetworkMode::Host.as_docker_arg(), "host");
}

#[test]
fn test_resource_limits() {
    let limits = ResourceLimits::default()
        .with_memory_mb(256)
        .with_cpu_percent(25)
        .with_timeout(Duration::from_secs(30));

    assert_eq!(limits.memory_bytes, 256 * 1024 * 1024);
    assert_eq!(limits.cpu_percent, 25);
    assert_eq!(limits.timeout, Duration::from_secs(30));

    let args = limits.to_docker_args();
    assert!(args.iter().any(|a| a.contains("--memory=")));
    assert!(args.iter().any(|a| a.contains("--cpu-quota=")));
    assert!(args.iter().any(|a| a.contains("--pids-limit=")));
}

#[test]
fn test_mount() {
    let ro_mount = Mount::read_only("/host/path", "/container/path");
    assert!(ro_mount.read_only);
    assert!(ro_mount.to_docker_arg().contains("readonly"));

    let rw_mount = Mount::read_write("/host/data", "/data");
    assert!(!rw_mount.read_only);
    assert!(!rw_mount.to_docker_arg().contains("readonly"));
}

#[test]
fn test_valid_env_name() {
    use docker::DockerSandbox;

    assert!(DockerSandbox::is_valid_env_name("PATH"));
    assert!(DockerSandbox::is_valid_env_name("MY_VAR"));
    assert!(DockerSandbox::is_valid_env_name("VAR123"));

    assert!(!DockerSandbox::is_valid_env_name(""));
    assert!(!DockerSandbox::is_valid_env_name("123VAR"));
    assert!(!DockerSandbox::is_valid_env_name("MY-VAR"));
    assert!(!DockerSandbox::is_valid_env_name("MY VAR"));
}

#[test]
fn test_sandbox_output() {
    let success = SandboxOutput::success("output");
    assert!(success.success);
    assert_eq!(success.exit_code, 0);

    let failure = SandboxOutput::failure("error", 1);
    assert!(!failure.success);
    assert_eq!(failure.exit_code, 1);
}

#[test]
fn test_sandbox_config_default() {
    let config = SandboxConfig::default();
    assert_eq!(config.policy, SandboxPolicy::Moderate);
    assert_eq!(config.default_network, NetworkMode::None);
    assert_eq!(config.runtime_preference, "auto");
    assert!(config.prefer_apple_container);
}

#[test]
fn test_container_runtime_display_name() {
    assert_eq!(ContainerRuntime::Docker.display_name(), "Docker");
    assert_eq!(
        ContainerRuntime::AppleContainer.display_name(),
        "Apple Container"
    );
    assert_eq!(ContainerRuntime::None.display_name(), "None (no isolation)");
}

#[test]
fn test_container_runtime_vm_isolation() {
    assert!(!ContainerRuntime::Docker.is_vm_isolated());
    assert!(ContainerRuntime::AppleContainer.is_vm_isolated());
    assert!(!ContainerRuntime::None.is_vm_isolated());
}

#[test]
fn test_network_mode_apple_container_arg() {
    assert_eq!(NetworkMode::None.as_apple_container_arg(), "none");
    assert_eq!(NetworkMode::Bridge.as_apple_container_arg(), "nat");
    assert_eq!(NetworkMode::Host.as_apple_container_arg(), "host");
}

#[test]
fn test_mount_apple_container_arg() {
    let ro_mount = Mount::read_only("/host/path", "/container/path");
    let arg = ro_mount.to_apple_container_arg();
    assert!(arg.contains("--mount="));
    assert!(arg.contains(":ro"));

    let rw_mount = Mount::read_write("/host/data", "/data");
    let arg = rw_mount.to_apple_container_arg();
    assert!(!arg.contains(":ro"));
}

#[test]
fn test_resource_limits_apple_container_args() {
    let limits = ResourceLimits::default()
        .with_memory_mb(256)
        .with_cpu_percent(50);

    let args = limits.to_apple_container_args();
    assert!(args.iter().any(|a| a.contains("--memory=")));
    assert!(args.iter().any(|a| a.contains("--cpus=")));
}

#[test]
fn test_unified_sandbox_with_runtime() {
    let config = SandboxConfig::default();

    let docker_sandbox = UnifiedSandbox::with_runtime(config.clone(), ContainerRuntime::Docker);
    assert_eq!(docker_sandbox.runtime(), ContainerRuntime::Docker);
    assert!(docker_sandbox.is_available());

    let apple_sandbox =
        UnifiedSandbox::with_runtime(config.clone(), ContainerRuntime::AppleContainer);
    assert_eq!(apple_sandbox.runtime(), ContainerRuntime::AppleContainer);
    assert!(apple_sandbox.is_available());

    let none_sandbox = UnifiedSandbox::with_runtime(config, ContainerRuntime::None);
    assert_eq!(none_sandbox.runtime(), ContainerRuntime::None);
    assert!(!none_sandbox.is_available());
}

#[test]
fn test_unified_sandbox_is_valid_env_name() {
    // Access through DockerSandbox since is_valid_env_name is private in UnifiedSandbox
    assert!(docker::DockerSandbox::is_valid_env_name("PATH"));
    assert!(docker::DockerSandbox::is_valid_env_name("MY_VAR"));
    assert!(docker::DockerSandbox::is_valid_env_name("VAR123"));

    assert!(!docker::DockerSandbox::is_valid_env_name(""));
    assert!(!docker::DockerSandbox::is_valid_env_name("123VAR"));
    assert!(!docker::DockerSandbox::is_valid_env_name("MY-VAR"));
}
