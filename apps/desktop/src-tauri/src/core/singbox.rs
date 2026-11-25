use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

pub fn patch(config: &mut Value, socks_port: u16, api_port: u16) -> Result<()> {
    let root = config
        .as_object_mut()
        .ok_or_else(|| anyhow!("sing-box config must be a JSON object"))?;

    patch_inbounds(root, socks_port);
    patch_api(root, api_port);
    Ok(())
}

pub fn write_config(config: &Value, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(config)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn patch_inbounds(root: &mut Map<String, Value>, socks_port: u16) {
    let mut inbounds = root.remove("inbounds").unwrap_or_else(|| json!([]));
    if !inbounds.is_array() {
        inbounds = json!([]);
    }
    let inbounds = inbounds.as_array_mut().expect("inbounds array");
    let mut patched = false;
    for inbound in inbounds.iter_mut() {
        let kind = inbound
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if matches!(kind, "mixed" | "socks") && !patched {
            inbound["listen"] = json!("127.0.0.1");
            inbound["listen_port"] = json!(socks_port);
            patched = true;
        }
    }
    if !patched {
        inbounds.push(json!({
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": socks_port
        }));
    }
    root.insert("inbounds".into(), json!(inbounds));
}

fn patch_api(root: &mut Map<String, Value>, api_port: u16) {
    let experimental = root.entry("experimental").or_insert_with(|| json!({}));
    if !experimental.is_object() {
        *experimental = json!({});
    }
    let experimental = experimental.as_object_mut().expect("experimental object");
    let api = experimental.entry("clash_api").or_insert_with(|| json!({}));
    if !api.is_object() {
        *api = json!({});
    }
    let api = api.as_object_mut().expect("clash api object");
    api.insert(
        "external_controller".into(),
        json!(format!("127.0.0.1:{api_port}")),
    );
    api.insert("secret".into(), json!("m3vpn"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_inbound_and_clash_api() {
        let mut config = json!({
            "inbounds": [{
                "type": "mixed",
                "tag": "mixed-in",
                "listen": "0.0.0.0",
                "listen_port": 10808
            }],
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });

        patch(&mut config, 20808, 29090).unwrap();

        assert_eq!(config["inbounds"][0]["listen"], "127.0.0.1");
        assert_eq!(config["inbounds"][0]["listen_port"], 20808);
        assert_eq!(
            config["experimental"]["clash_api"]["external_controller"],
            "127.0.0.1:29090"
        );
    }

    #[test]
    fn does_not_mark_singbox_sockets() {
        let mut config = json!({ "outbounds": [] });
        patch(&mut config, 20808, 29090).unwrap();
        assert!(config
            .get("route")
            .and_then(|route| route.get("default_mark"))
            .is_none());
    }

    #[test]
    fn adds_mixed_inbound_when_missing() {
        let mut config = json!({ "outbounds": [] });
        patch(&mut config, 20808, 29090).unwrap();
        assert_eq!(config["inbounds"][0]["type"], "mixed");
        assert_eq!(config["inbounds"][0]["listen_port"], 20808);
    }
}
