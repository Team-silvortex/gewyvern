use super::super::docker_target::{OwnedResource, docker, target_addresses};
use super::super::http_evidence::sha256;
use super::super::{ValidationError, repo_root, tool};
use serde_json::{Value, json};
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

const CONFIG: &str = include_str!("../../../../../ebpf/validation/stun/turnserver.conf");
const ENTRY: [&str; 3] = ["/usr/bin/turnserver", "-c", "/turnserver.conf"];

pub(super) struct Target {
    container: OwnedResource,
    network: OwnedResource,
    pub source: Ipv4Addr,
    pub address: SocketAddr,
    pub metadata: Value,
}
impl Target {
    pub fn start(token: &str, out: &Path) -> Result<Self, ValidationError> {
        let image = String::from_utf8(tool(
            docker().args([
                "image",
                "inspect",
                "gewyvern-field-stun:4.10.0",
                "--format",
                "{{.Id}}",
            ]),
            out,
        )?)
        .map_err(|e| ValidationError::new(e.to_string()))?;
        if !image.trim().starts_with("sha256:") {
            return Err(ValidationError::new("invalid STUN image identity"));
        }
        let network =
            OwnedResource::create("network", token, &["--driver", "bridge", "--internal"], out)?;
        let network_info = network.inspect()?;
        let container = OwnedResource::create(
            "container",
            token,
            &[
                "--pull=never",
                "--network",
                &network.name,
                "--user=65534:65534",
                "--cpus=1",
                "--memory=128m",
                "--pids-limit=64",
                "--read-only",
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,size=4m",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                image.trim(),
            ],
            out,
        )?;
        let before = container.inspect()?;
        if before["Config"]["Entrypoint"] != json!(ENTRY)
            || before["HostConfig"]["Privileged"] != false
            || before["HostConfig"]["CapDrop"] != json!(["ALL"])
            || before["HostConfig"]["ReadonlyRootfs"] != true
        {
            return Err(ValidationError::new("STUN target isolation differs"));
        }
        tool(
            docker()
                .args(["cp", &format!("{}:/turnserver.conf", container.name)])
                .arg(out.join("turnserver.conf")),
            out,
        )?;
        if fs::read(out.join("turnserver.conf"))? != CONFIG.as_bytes()
            || fs::read(repo_root().join("ebpf/validation/stun/turnserver.conf"))?
                != CONFIG.as_bytes()
        {
            return Err(ValidationError::new(
                "STUN image configuration differs from compiled fixture",
            ));
        }
        tool(docker().args(["start", &container.name]), out)?;
        let info = container.inspect()?;
        if info["Mounts"].as_array().is_none_or(|m| !m.is_empty()) {
            return Err(ValidationError::new("STUN target has unexpected mounts"));
        }
        let (source, destination) = target_addresses(&info, &network_info, &network.name)?;
        Ok(Self {
            metadata: json!({"image_id":image.trim(),"container_id":info["Id"],"network_id":network_info["Id"],"config_sha256":sha256(CONFIG.as_bytes()),"internal_network":true,"published_ports":false,"host_mounts":false,"turn_relay_enabled":false}),
            container,
            network,
            source,
            address: SocketAddr::from((destination, 3478)),
        })
    }
    pub fn log(&self, out: &Path) -> Result<(), ValidationError> {
        fs::write(
            out.join("startup.log"),
            tool(
                docker().args(["logs", "--tail", "80", &self.container.name]),
                out,
            )?,
        )?;
        Ok(())
    }
    pub fn cleanup(&mut self) -> Result<(), ValidationError> {
        self.container.cleanup()?;
        self.network.cleanup()
    }
}
