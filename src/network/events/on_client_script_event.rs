use bevy::prelude::Res;
use common::plugin_api::events::client_script_event::ClientScriptEvent;
use common::utils::events::EventReader;

use crate::{network::server::NetworkEventListener, plugins::plugins_manager::PluginsManager};

pub fn on_client_script_event(
    events: Res<NetworkEventListener<ClientScriptEvent>>,
    plugins_manager: Res<PluginsManager>,
) {
    let _s = crate::span!("events.on_client_script_event");
    for event in events.0.iter_events() {
        plugins_manager.dispatch_client_script_event(&event);
    }
}
