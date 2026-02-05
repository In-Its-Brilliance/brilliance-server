use std::path::PathBuf;

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

    pub fn call_on_enable(&mut self) -> Result<(), String> {
        let plugin = self.instance.as_mut().ok_or("plugin not initialized".to_string())?;
        plugin
            .call::<&str, &str>("on_enable", "")
            .map(|_| ())
            .map_err(|e| format!("{}", e))
    }

    pub fn call_on_disable(&mut self) -> Result<(), String> {
        let plugin = self.instance.as_mut().ok_or("plugin not initialized".to_string())?;
        plugin
            .call::<&str, &str>("on_disable", "")
            .map(|_| ())
            .map_err(|e| format!("{}", e))
    }
}
