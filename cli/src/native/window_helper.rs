//! Opt-in XFP window control. Only used for daemon-owned headed Chrome profiles.
//! CDP controls page automation; the extension selects tabs without activating windows.
use super::cdp::client::CdpClient;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn prepare(profile: Option<&str>) -> Result<PathBuf, String> {
    let profile = profile.ok_or("Background window control requires an explicit profile path")?;
    if !Path::new(profile).is_absolute() {
        return Err("Background window control requires an absolute profile path".into());
    }
    let directory = Path::new(profile).join(".xfp-window-helper");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    for (name, contents) in [
        ("manifest.json", include_str!("window-helper/manifest.json")),
        ("worker.js", include_str!("window-helper/worker.js")),
    ] {
        std::fs::write(directory.join(name), contents).map_err(|e| e.to_string())?;
    }
    Ok(directory)
}

pub struct WindowHelper {
    session_id: String,
}

impl WindowHelper {
    /// Attach to the helper already installed over the trusted browser pipe.
    pub async fn attach(client: &CdpClient, id: &str) -> Result<Self, String> {
        let worker_url = format!("chrome-extension://{id}/worker.js");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let targets = client.send_command("Target.getTargets", None, None).await?;
            if let Some(target) = targets["targetInfos"].as_array().and_then(|ts| {
                ts.iter()
                    .find(|t| t["type"] == "service_worker" && t["url"] == worker_url)
            }) {
                let attached = client
                    .send_command(
                        "Target.attachToTarget",
                        Some(json!({"targetId": target["targetId"], "flatten": true})),
                        None,
                    )
                    .await?;
                let helper = Self {
                    session_id: attached["sessionId"]
                        .as_str()
                        .ok_or("Missing helper session")?
                        .into(),
                };
                // Target discovery may precede execution of the worker's top-level script.
                while helper
                    .evaluate(client, "typeof globalThis.xfpWindow === 'object'".into())
                    .await?
                    != json!(true)
                {
                    if tokio::time::Instant::now() >= deadline {
                        return Err("Window helper initialization timed out".into());
                    }
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                // Keep the CDP session attached so the worker stays alive throughout the browser session.
                helper
                    .evaluate(client, "globalThis.xfpWindow.ensureWindow()".into())
                    .await?;
                return Ok(helper);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err("Window helper did not start".into());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn select(&self, client: &CdpClient, target: &str) -> Result<(), String> {
        self.evaluate(
            client,
            format!("globalThis.xfpWindow.select({})", json!(target)),
        )
        .await?;
        Ok(())
    }

    async fn evaluate(&self, client: &CdpClient, expression: String) -> Result<Value, String> {
        let result = client
            .send_command(
                "Runtime.evaluate",
                Some(
                    json!({"expression": expression, "awaitPromise": true, "returnByValue": true}),
                ),
                Some(&self.session_id),
            )
            .await?;
        if result.get("exceptionDetails").is_some() {
            return Err(format!(
                "Window helper failed: {}",
                result["exceptionDetails"]
            ));
        }
        Ok(result["result"]["value"].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_source_profile_names() {
        assert!(prepare(None).is_err());
        assert!(prepare(Some("Default")).is_err());
        assert!(prepare(Some("Profile 1")).is_err());
    }

    #[test]
    fn helper_has_no_site_access_or_debugger_attachment() {
        let manifest: Value =
            serde_json::from_str(include_str!("window-helper/manifest.json")).unwrap();
        assert_eq!(manifest["permissions"], json!(["debugger"]));
        assert!(manifest.get("host_permissions").is_none());
        let worker = include_str!("window-helper/worker.js");
        assert!(!worker.contains("chrome.debugger.attach"));
        assert!(!worker.contains("focused: true"));
        assert!(worker.contains("chrome.debugger.getTargets()"));
    }
}
