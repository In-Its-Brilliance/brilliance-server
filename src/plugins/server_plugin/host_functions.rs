use extism::*;
use std::sync::{Arc, Mutex};

pub type SharedHostContext = Arc<Mutex<HostContext>>;

#[derive(Default)]
pub struct HostContext {
    plugin_slug: String,
    world_generators: Vec<String>,
    has_on_chunk_generate: bool,
}

impl HostContext {
    pub fn create(plugin_slug: String) -> Self {
        Self {
            plugin_slug,
            ..Default::default()
        }
    }

    pub fn get_plugin_slug(&self) -> &String {
        &self.plugin_slug
    }

    pub fn get_world_generators(&self) -> &Vec<String> {
        &self.world_generators
    }

    pub(crate) fn set_has_on_chunk_generate(&mut self) {
        self.has_on_chunk_generate = true;
    }
}

pub fn register_world_generator_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let name: String = plugin.memory_get_val(&inputs[0])?;
    let inner = user_data.get()?;
    let inner = inner.lock().unwrap();
    let mut ctx = inner.lock().unwrap();

    if !ctx.has_on_chunk_generate {
        return Err(Error::msg(
            "Plugin must implement event for ChunkGenerateEvent to register world generator!",
        ));
    }
    log::debug!(target: "plugin", "[Host] Plugin '{}' registered generator: {}", ctx.plugin_slug, name);
    ctx.world_generators.push(name);
    plugin.memory_set_val(&mut outputs[0], "")?;
    Ok(())
}

pub fn get_plugin_slug_raw(
    plugin: &mut CurrentPlugin,
    _inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let inner = user_data.get()?;
    let inner = inner.lock().unwrap();
    let ctx = inner.lock().unwrap();
    plugin.memory_set_val(&mut outputs[0], ctx.get_plugin_slug())?;
    Ok(())
}

pub fn register_all<'a>(builder: PluginBuilder<'a>, ctx: &SharedHostContext) -> PluginBuilder<'a> {
    let ctx1 = Arc::clone(ctx);
    let ctx2 = Arc::clone(ctx);

    builder
        .with_function(
            "register_world_generator_raw",
            [PTR],
            [PTR],
            UserData::new(ctx1),
            register_world_generator_raw,
        )
        .with_function(
            "get_plugin_slug_raw",
            [],
            [PTR],
            UserData::new(ctx2),
            get_plugin_slug_raw,
        )
}
