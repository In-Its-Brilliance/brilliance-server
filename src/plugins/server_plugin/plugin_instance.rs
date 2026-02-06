use std::path::PathBuf;

use common::plugin_api::events::{plugin_load::PluginLoadEvent, plugin_unload::PluginUnloadEvent, PluginEvent};

pub struct PluginInstance {
    pub instance: Option<extism::Plugin>,
}

impl PluginInstance {
    pub fn new(wasm_path: &PathBuf) -> Result<Self, String> {
        let wasm = extism::Wasm::file(wasm_path);
        let manifest = extism::Manifest::new([wasm]);
        let plugin = extism::Plugin::new(&manifest, [], true).map_err(|e| format!("WASM init failed: {}", e))?;
        Ok(Self { instance: Some(plugin) })
    }

    pub fn call_event<E: PluginEvent + serde::Serialize>(&mut self, event: &E) -> Result<(), String> {
        let plugin = self.instance.as_mut().ok_or("plugin not initialized")?;
        let input = serde_json::to_string(event).map_err(|e| e.to_string())?;

        plugin
            .call::<&str, &str>(E::EXPORT_NAME, &input)
            .map(|_| ())
            .map_err(|e| format!("{}", e))
    }

    pub fn call_event_with_result<E, R>(&mut self, event: &E) -> Result<R, String>
    where
        E: PluginEvent + serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let plugin = self.instance.as_mut().ok_or("plugin not initialized")?;
        let input = serde_json::to_string(event).map_err(|e| e.to_string())?;

        let output = plugin
            .call::<&str, &str>(E::EXPORT_NAME, &input)
            .map_err(|e| format!("{}", e))?;

        serde_json::from_str(output).map_err(|e| e.to_string())
    }

    pub fn has_event_handler<E: PluginEvent>(&self) -> bool {
        self.instance
            .as_ref()
            .map(|p| p.function_exists(E::EXPORT_NAME))
            .unwrap_or(false)
    }

    pub fn call_on_enable(&mut self, slug: &str) -> Result<(), String> {
        let event = PluginLoadEvent {
            plugin_slug: slug.to_string(),
        };
        self.call_event(&event)
    }

    pub fn call_on_disable(&mut self, slug: &str) -> Result<(), String> {
        let event = PluginUnloadEvent {
            plugin_slug: slug.to_string(),
        };
        self.call_event(&event)
    }
}
