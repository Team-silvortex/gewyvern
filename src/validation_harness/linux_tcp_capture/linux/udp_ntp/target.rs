use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use super::super::docker_target::{OwnedResource, docker, target_addresses};
use super::super::http_evidence::sha256;
use super::super::{ValidationError, repo_root, tool};
use serde_json::{Value, json};

pub(super) const IMAGE: &str = "gewyvern-field-ntp:4.8";
const LOCAL: &str = include_str!("../../../../../ebpf/validation/ntp/local.conf");
const UNSYNCED: &str = include_str!("../../../../../ebpf/validation/ntp/unsynced.conf");
const ENTRY: [&str; 6] = ["/usr/sbin/chronyd", "-d", "-x", "-u", "root", "-4"];

fn clock_isolated(info: &Value) -> bool {
    let caps = info["HostConfig"]["CapAdd"].as_array();
    info["Config"]["Entrypoint"] == json!(ENTRY)
        && info["HostConfig"]["Privileged"] == false
        && info["HostConfig"]["CapDrop"] == json!(["ALL"])
        && info["HostConfig"]["ReadonlyRootfs"] == true
        && caps.is_some_and(|caps| {
            caps.len() == 1
                && matches!(
                    caps[0].as_str(),
                    Some("NET_BIND_SERVICE" | "CAP_NET_BIND_SERVICE")
                )
        })
}

pub(super) struct Target {
    container: OwnedResource,
    network: OwnedResource,
    pub source: Ipv4Addr,
    pub address: SocketAddr,
    pub metadata: Value,
}

impl Target {
    pub fn start(token: &str, local: bool, out: &Path) -> Result<Self, ValidationError> {
        let image = String::from_utf8(tool(
            docker().args(["image", "inspect", IMAGE, "--format", "{{.Id}}"]),
            out,
        )?)
        .map_err(|e| ValidationError::new(e.to_string()))?;
        if !image.trim().starts_with("sha256:") {
            return Err(ValidationError::new("invalid NTP image identity"));
        }
        let network =
            OwnedResource::create("network", token, &["--driver", "bridge", "--internal"], out)?;
        let net_info = network.inspect()?;
        let file = if local { "local.conf" } else { "unsynced.conf" };
        let config = if local { LOCAL } else { UNSYNCED };
        let container = OwnedResource::create(
            "container",
            token,
            &[
                "--pull=never",
                "--network",
                &network.name,
                "--cpus=1",
                "--memory=64m",
                "--pids-limit=64",
                "--read-only",
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,size=4m",
                "--cap-drop=ALL",
                "--cap-add=NET_BIND_SERVICE",
                "--security-opt=no-new-privileges",
                image.trim(),
                "-f",
                &format!("/{file}"),
            ],
            out,
        )?;
        let before = container.inspect()?;
        if !clock_isolated(&before) {
            return Err(ValidationError::new(
                "NTP target clock-control isolation differs from fixture",
            ));
        }
        tool(
            docker()
                .args(["cp", &format!("{}:/{file}", container.name)])
                .arg(out.join(file)),
            out,
        )?;
        if fs::read(out.join(file))? != config.as_bytes()
            || fs::read(repo_root().join("ebpf/validation/ntp").join(file))? != config.as_bytes()
        {
            return Err(ValidationError::new(
                "NTP image configuration differs from compiled fixture",
            ));
        }
        tool(docker().args(["start", &container.name]), out)?;
        let info = container.inspect()?;
        if info["Mounts"].as_array().is_none_or(|m| !m.is_empty()) {
            return Err(ValidationError::new("NTP target has unexpected mounts"));
        }
        let (source, destination) = target_addresses(&info, &net_info, &network.name)?;
        Ok(Self {
            metadata: json!({"image_id":image.trim(),"container_id":info["Id"],"network_id":net_info["Id"],"config_sha256":sha256(config.as_bytes()),
            "entrypoint":ENTRY,"internal_network":true,"published_ports":false,"host_mounts":false,"clock_control_disabled":true,"sys_time_capability":false,"external_time_sources":false}),
            container,
            network,
            source,
            address: SocketAddr::from((destination, 123)),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn docker_capability_spelling_never_broadens_clock_authority() {
        let good = json!({"Config":{"Entrypoint":ENTRY},"HostConfig":{"Privileged":false,"CapDrop":["ALL"],"CapAdd":["CAP_NET_BIND_SERVICE"],"ReadonlyRootfs":true}});
        assert!(clock_isolated(&good));
        let mut short = good.clone();
        short["HostConfig"]["CapAdd"] = json!(["NET_BIND_SERVICE"]);
        assert!(clock_isolated(&short));
        for caps in [
            json!(["CAP_SYS_TIME"]),
            json!(["CAP_NET_BIND_SERVICE", "CAP_SYS_TIME"]),
            json!(["ALL"]),
            json!([]),
            json!(null),
        ] {
            let mut bad = good.clone();
            bad["HostConfig"]["CapAdd"] = caps;
            assert!(!clock_isolated(&bad));
        }
        for field in ["Privileged", "ReadonlyRootfs"] {
            let mut bad = good.clone();
            bad["HostConfig"][field] = json!(field == "Privileged");
            assert!(!clock_isolated(&bad));
        }
        let mut bad = good.clone();
        bad["Config"]["Entrypoint"] = json!(["/usr/sbin/chronyd", "-d"]);
        assert!(!clock_isolated(&bad));
    }
}
