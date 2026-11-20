use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::{ValidationError, tool};

pub(super) const IMAGE: &str =
    "bkimminich/juice-shop@sha256:e68144772ebaaca0ec117b38d44903af92416793230288ef7c5437fc4f26850a";
const OWNER_LABEL: &str = "org.silvortex.gewyvern.capture-run";

pub(super) fn docker() -> Command {
    let mut command = Command::new("docker");
    // BPF observes this host, never a daemon selected by a remote Docker context.
    command.args(["--host", "unix:///var/run/docker.sock"]);
    command.env_remove("DOCKER_CONTEXT");
    command.env_remove("DOCKER_TLS_VERIFY");
    command
}

pub(super) struct OwnedResource {
    kind: &'static str,
    pub(super) name: String,
    token: String,
    out: PathBuf,
    removed: bool,
}

impl OwnedResource {
    pub(super) fn create(
        kind: &'static str,
        token: &str,
        args: &[&str],
        out: &Path,
    ) -> Result<Self, ValidationError> {
        if !matches!(kind, "container" | "network") {
            return Err(ValidationError::new("unsupported Docker resource kind"));
        }
        let resource = Self {
            kind,
            name: format!("gewyvern-capture-{token}-{kind}"),
            token: token.into(),
            out: out.into(),
            removed: false,
        };
        let mut command = docker();
        command.args([kind, "create"]);
        if kind == "container" {
            command.args(["--name", &resource.name]);
        }
        command.args(["--label", &format!("{OWNER_LABEL}={token}")]);
        command.args(args);
        if kind == "network" {
            command.arg(&resource.name);
        }
        // The guard exists before creation: even an ambiguous command failure
        // checks the unique ownership label before removing anything.
        tool(&mut command, out)?;
        resource.inspect()?;
        Ok(resource)
    }

    pub(super) fn inspect(&self) -> Result<Value, ValidationError> {
        let bytes = tool(docker().args([self.kind, "inspect", &self.name]), &self.out)?;
        let values: Vec<Value> = serde_json::from_slice(&bytes)?;
        let [value] = values.as_slice() else {
            return Err(ValidationError::new("ambiguous owned Docker resource"));
        };
        owned_id(value, self.kind, &self.name, &self.token)?;
        Ok(value.clone())
    }

    pub(super) fn cleanup(&mut self) -> Result<(), ValidationError> {
        let value = self.inspect()?;
        let id = owned_id(&value, self.kind, &self.name, &self.token)?;
        let mut command = docker();
        command.args([self.kind, "rm"]);
        if self.kind == "container" {
            command.args(["--force", "--volumes"]);
        }
        tool(command.arg(id), &self.out)?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for OwnedResource {
    fn drop(&mut self) {
        if !self.removed
            && let Err(error) = self.cleanup()
        {
            eprintln!("owned Docker cleanup failed for {}: {error}", self.name);
        }
    }
}

fn owned_id<'a>(
    value: &'a Value,
    kind: &str,
    name: &str,
    token: &str,
) -> Result<&'a str, ValidationError> {
    let (labels, expected_name) = match kind {
        "container" => (&value["Config"]["Labels"], format!("/{name}")),
        "network" => (&value["Labels"], name.into()),
        _ => return Err(ValidationError::new("unsupported Docker resource kind")),
    };
    let id = value["Id"].as_str().unwrap_or_default();
    if value["Name"].as_str() != Some(&expected_name)
        || labels[OWNER_LABEL].as_str() != Some(token)
        || id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ValidationError::new(
            "Docker resource ownership mismatch; refusing cleanup",
        ));
    }
    Ok(id)
}

pub(super) fn target_addresses(
    value: &Value,
    network: &Value,
    name: &str,
) -> Result<(Ipv4Addr, Ipv4Addr), ValidationError> {
    target_network(value, network, name)?;
    let networks = &value["NetworkSettings"]["Networks"];
    let configs = network["IPAM"]["Config"]
        .as_array()
        .ok_or_else(|| ValidationError::new("missing owned network subnet"))?;
    let [config] = configs.as_slice() else {
        return Err(ValidationError::new("ambiguous owned network subnet"));
    };
    ipv4_addresses(&networks[name]["IPAddress"], config)
}

fn target_network(value: &Value, network: &Value, name: &str) -> Result<(), ValidationError> {
    let networks = value["NetworkSettings"]["Networks"]
        .as_object()
        .ok_or_else(|| ValidationError::new("missing target network attachment"))?;
    let bindings = &value["HostConfig"]["PortBindings"];
    if networks.len() != 1
        || networks.get(name).is_none()
        || value["HostConfig"]["NetworkMode"] != name
        || value["State"]["Running"] != true
        || network["Internal"] != true
        || network["Driver"] != "bridge"
        || networks[name]["NetworkID"] != network["Id"]
        || (!bindings.is_null() && !bindings.as_object().is_some_and(|ports| ports.is_empty()))
    {
        return Err(ValidationError::new(
            "target must use only its owned internal bridge without published ports",
        ));
    }
    Ok(())
}

fn ipv4_addresses(
    address: &Value,
    config: &Value,
) -> Result<(Ipv4Addr, Ipv4Addr), ValidationError> {
    let parse = |value: &Value| {
        value
            .as_str()
            .and_then(|text| text.parse::<Ipv4Addr>().ok())
            .filter(|ip| ip.is_private())
            .ok_or_else(|| ValidationError::new("target addresses must be private IPv4"))
    };
    let source = parse(&config["Gateway"])?;
    let destination = parse(address)?;
    let (subnet, prefix) = config["Subnet"]
        .as_str()
        .and_then(|text| text.split_once('/'))
        .ok_or_else(|| ValidationError::new("invalid owned subnet"))?;
    let subnet = subnet
        .parse::<Ipv4Addr>()
        .map_err(|_| ValidationError::new("invalid subnet address"))?;
    let prefix = prefix
        .parse::<u32>()
        .ok()
        .filter(|prefix| (8..=30).contains(prefix))
        .ok_or_else(|| ValidationError::new("invalid subnet prefix"))?;
    let mask = u32::MAX << (32 - prefix);
    if source == destination
        || u32::from(source) & mask != u32::from(subnet) & mask
        || u32::from(destination) & mask != u32::from(subnet) & mask
    {
        return Err(ValidationError::new(
            "target addresses are outside the owned subnet",
        ));
    }
    Ok((source, destination))
}

pub(super) fn target_ipv6_addresses(
    value: &Value,
    network: &Value,
    name: &str,
) -> Result<(Ipv6Addr, Ipv6Addr), ValidationError> {
    target_network(value, network, name)?;
    let attachment = &value["NetworkSettings"]["Networks"][name];
    if network["EnableIPv6"] != true
        || network["EnableIPv4"] != false
        || attachment["IPAddress"] != ""
        || attachment["GlobalIPv6PrefixLen"] != 64
    {
        return Err(ValidationError::new(
            "target must use IPv6-only /64 networking",
        ));
    }
    let configs = network["IPAM"]["Config"]
        .as_array()
        .ok_or_else(|| ValidationError::new("missing IPv6 subnet"))?;
    let [config] = configs.as_slice() else {
        return Err(ValidationError::new("ambiguous IPv6 subnet"));
    };
    let parse = |text: &str| {
        text.parse::<Ipv6Addr>()
            .ok()
            .filter(Ipv6Addr::is_unique_local)
            .ok_or_else(|| ValidationError::new("target addresses must be unique-local IPv6"))
    };
    let source = parse(config["Gateway"].as_str().unwrap_or_default())?;
    let destination = parse(attachment["GlobalIPv6Address"].as_str().unwrap_or_default())?;
    let subnet = config["Subnet"]
        .as_str()
        .unwrap_or_default()
        .strip_suffix("/64")
        .ok_or_else(|| ValidationError::new("expected owned IPv6 /64"))?;
    let subnet = parse(subnet)?;
    let mask = u128::MAX << 64;
    if source == destination
        || u128::from(subnet) & !mask != 0
        || u128::from(source) & mask != u128::from(subnet)
        || u128::from(destination) & mask != u128::from(subnet)
    {
        return Err(ValidationError::new(
            "target IPv6 addresses are outside the owned subnet",
        ));
    }
    Ok((source, destination))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ipv6_endpoint_requires_owned_ula_only_network_and_full_prefix_match() {
        let value = json!({"State":{"Running":true}, "HostConfig":{"NetworkMode":"owned", "PortBindings":{}},
            "NetworkSettings":{"Networks":{"owned":{"NetworkID":"network-id", "IPAddress":"",
                "GlobalIPv6Address":"fd01:2345:6789::2", "GlobalIPv6PrefixLen":64}}}});
        let network = json!({"Id":"network-id", "Internal":true, "Driver":"bridge",
            "EnableIPv6":true,"EnableIPv4":false,
            "IPAM":{"Config":[{"Gateway":"fd01:2345:6789::1","Subnet":"fd01:2345:6789::/64"}]}});
        assert_eq!(
            target_ipv6_addresses(&value, &network, "owned").unwrap(),
            (
                "fd01:2345:6789::1".parse().unwrap(),
                "fd01:2345:6789::2".parse().unwrap()
            )
        );
        for address in [
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "fe80::2",
            "2001:db8::2",
            "fd02:2345:6789::2",
            "fd01:2345:6789:1::2",
            "fd01:2345:6789::1",
        ] {
            let mut bad = value.clone();
            bad["NetworkSettings"]["Networks"]["owned"]["GlobalIPv6Address"] = json!(address);
            assert!(target_ipv6_addresses(&bad, &network, "owned").is_err());
        }
        for kind in 0..8 {
            let (mut bad, mut net) = (value.clone(), network.clone());
            match kind {
                0 => net["EnableIPv4"] = json!(true),
                1 => net["EnableIPv6"] = json!(false),
                2 => net["Internal"] = json!(false),
                3 => net["IPAM"]["Config"][0]["Subnet"] = json!("fd01:2345:6789::/48"),
                4 => net["IPAM"]["Config"][0]["Subnet"] = json!("fd01:2345:6789::1/64"),
                5 => bad["NetworkSettings"]["Networks"]["owned"]["IPAddress"] = json!("172.20.0.2"),
                6 => bad["NetworkSettings"]["Networks"]["owned"]["NetworkID"] = json!("foreign"),
                _ => bad["HostConfig"]["PortBindings"] = json!({"53/udp":[{"HostPort":"53"}]}),
            }
            assert!(target_ipv6_addresses(&bad, &net, "owned").is_err());
        }
    }

    #[test]
    fn cleanup_requires_exact_name_id_and_run_label() {
        assert!(OwnedResource::create("volume", "unused", &[], Path::new("unused")).is_err());
        let value = json!({"Id":"a".repeat(64), "Name":"/owned", "Config":{"Labels":{OWNER_LABEL:"nonce"}}});
        assert!(owned_id(&value, "container", "owned", "nonce").is_ok());
        assert!(owned_id(&value, "container", "owned", "other").is_err());
        assert!(owned_id(&value, "container", "foreign", "nonce").is_err());
        assert!(owned_id(&value, "volume", "owned", "nonce").is_err());
        for id in ["", "--all", "short", &"g".repeat(64)] {
            let mut bad = value.clone();
            bad["Id"] = json!(id);
            assert!(owned_id(&bad, "container", "owned", "nonce").is_err());
        }
        let network = json!({"Id":"b".repeat(64), "Name":"owned", "Labels":{OWNER_LABEL:"nonce"}});
        assert!(owned_id(&network, "network", "owned", "nonce").is_ok());
    }

    #[test]
    fn endpoint_rejects_public_addresses_published_ports_and_foreign_networks() {
        let value = json!({"State":{"Running":true}, "HostConfig":{"NetworkMode":"owned", "PortBindings":{}},
            "NetworkSettings":{"Networks":{"owned":{"NetworkID":"network-id", "IPAddress":"172.20.0.2"}}}});
        let network = json!({"Id":"network-id", "Internal":true,"Driver":"bridge", "IPAM":{"Config":[{"Gateway":"172.20.0.1", "Subnet":"172.20.0.0/16"}]}});
        assert_eq!(
            target_addresses(&value, &network, "owned").unwrap(),
            (Ipv4Addr::new(172, 20, 0, 1), Ipv4Addr::new(172, 20, 0, 2))
        );
        assert!(target_addresses(&value, &network, "foreign").is_err());
        for ip in ["0.0.0.0", "::", "192.0.2.1", "10.0.0.2", "172.20.0.1"] {
            let mut bad = value.clone();
            bad["NetworkSettings"]["Networks"]["owned"]["IPAddress"] = json!(ip);
            assert!(target_addresses(&bad, &network, "owned").is_err());
        }
        let mut public = value.clone();
        public["HostConfig"]["PortBindings"] =
            json!({"3000/tcp":[{"HostIp":"0.0.0.0","HostPort":"3000"}]});
        assert!(target_addresses(&public, &network, "owned").is_err());
    }
}
