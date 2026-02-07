use std::{path::PathBuf, sync::Arc};

use common::plugin_api::events::{generage_chunk::ChunkGenerateEvent, PluginEvent};
use parking_lot::Mutex;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::plugins::server_plugin::host_functions::{self, HostContext, SharedHostContext};

#[derive(Default)]
pub struct WASMPluginManager {
    instances: Vec<Mutex<PluginInstance>>,
}

impl WASMPluginManager {
    pub fn new(wasm_path: &PathBuf, slug: &str, pool_size: usize) -> Result<Self, String> {
        let mut config = wasmtime::Config::new();
        config.wasm_backtrace(false);

        let primary = PluginInstance::new(wasm_path, slug, config.clone()).map(Mutex::new)?;

        let rest: Result<Vec<_>, String> = (1..pool_size)
            .into_par_iter()
            .map(|_| PluginInstance::new(wasm_path, slug, config.clone()).map(Mutex::new))
            .collect();

        let mut instances = vec![primary];
        instances.extend(rest?);

        Ok(Self {
            instances,
        })
    }

    /// Первый инстанс для lifecycle событий (on_load, on_unload)
    pub fn primary(&self) -> parking_lot::MutexGuard<'_, PluginInstance> {
        self.instances[0].lock()
    }

    /// Свободный инстанс для параллельных вызовов
    pub fn acquire(&self) -> parking_lot::MutexGuard<'_, PluginInstance> {
        for inst in &self.instances {
            if let Some(guard) = inst.try_lock() {
                return guard;
            }
        }
        self.instances[0].lock()
    }

    pub fn call_event<E: PluginEvent + serde::Serialize>(&self, event: &E) -> Result<(), String> {
        self.primary().call_event(event)
    }

    pub fn call_event_with_result<E, R>(&self, event: &E) -> Result<R, String>
    where
        E: PluginEvent + serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        self.acquire().call_event_with_result(event)
    }

    pub fn has_world_generator(&self, method: &String) -> bool {
        self.primary().has_world_generator(method)
    }
}

pub struct PluginInstance {
    pub instance: Option<extism::Plugin>,
    host_context: SharedHostContext,
}

impl PluginInstance {
    pub fn new(wasm_path: &PathBuf, slug: &str, config: wasmtime::Config) -> Result<Self, String> {
        let wasm = extism::Wasm::file(wasm_path);
        let manifest = extism::Manifest::new([wasm]);

        let ctx: SharedHostContext = Arc::new(Mutex::new(HostContext::create(slug.to_string())));

        let builder = extism::PluginBuilder::new(manifest)
            .with_wasi(true)
            .with_wasmtime_config(config);
        let builder = host_functions::register_all(builder, &ctx);
        let plugin = builder.build().map_err(|e| format!("WASM init failed: {}", e))?;

        if plugin.function_exists(ChunkGenerateEvent::EXPORT_NAME) {
            ctx.lock().set_has_on_chunk_generate();
        }

        Ok(Self {
            instance: Some(plugin),
            host_context: ctx,
        })
    }

    pub fn call_event<E: PluginEvent + serde::Serialize>(&mut self, event: &E) -> Result<(), String> {
        let plugin = self.instance.as_mut().ok_or("plugin not initialized")?;
        let input = serde_json::to_string(event).map_err(|e| e.to_string())?;

        match plugin.call::<&str, &str>(E::EXPORT_NAME, &input) {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.root_cause().to_string();
                Err(format!("&cEvent &4\"{}\"&c error:\n{}", E::EXPORT_NAME, msg))
            }
        }
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

    pub fn has_world_generator(&self, method: &String) -> bool {
        let ctx = self.host_context.lock();
        ctx.get_world_generators().contains(method)
    }
}
