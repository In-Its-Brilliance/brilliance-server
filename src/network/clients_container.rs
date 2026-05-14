use ahash::AHashMap;
use bevy_ecs::resource::Resource;
use common::utils::debug::SmartRwLock;
use network::NetworkServerConnection;
use std::sync::Arc;

use super::client_network::ClientNetwork;

#[derive(Resource)]
pub struct ClientsContainer {
    players: AHashMap<u64, ClientNetwork>,
}

#[derive(Resource, Clone)]
pub struct SharedClientsContainer {
    inner: Arc<SmartRwLock<ClientsContainer>>,
}

impl SharedClientsContainer {
    pub fn new(inner: Arc<SmartRwLock<ClientsContainer>>) -> Self {
        Self { inner }
    }

    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, ClientsContainer> {
        self.inner.read()
    }

    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, ClientsContainer> {
        self.inner.write()
    }

    pub fn clone_inner(&self) -> Arc<SmartRwLock<ClientsContainer>> {
        self.inner.clone()
    }
}

impl Default for ClientsContainer {
    fn default() -> Self {
        Self {
            players: AHashMap::new(),
        }
    }
}

impl ClientsContainer {
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, u64, ClientNetwork> {
        self.players.iter()
    }

    pub fn add(&mut self, connection: NetworkServerConnection) {
        let network = ClientNetwork::new(connection);
        self.players.insert(network.get_client_id(), network);
    }

    pub fn remove(&mut self, client_id: &u64) {
        self.players.remove(client_id);
    }

    pub fn disconnect_all(&mut self, message: Option<String>) {
        for (_client_id, client) in self.players.drain() {
            client.disconnect(message.clone());
        }
    }

    pub fn get(&self, key: &u64) -> Option<&ClientNetwork> {
        self.players.get(key)
    }

    pub fn get_by_login(&self, login: &String) -> Option<&ClientNetwork> {
        let mut client_id: Option<u64> = None;

        for (_client_id, client) in self.players.iter() {
            let Some(info) = client.get_client_info() else {
                continue;
            };
            if info.get_login() == login {
                client_id = Some(_client_id.clone());
                break;
            }
        }
        let Some(client_id) = client_id else {
            return None;
        };
        self.players.get(&client_id)
    }
}
