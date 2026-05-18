# WASM

Logs method: `extism_pdk::log!`


# Events list

## `PluginLoadEvent`

**Returns:** `Result<(), Error>`

  - `get_slug() -> Result<String, Error>`
  - `register_world_generator(name: &str) -> Result<(), Error>`

## `PluginUnloadEvent`

## `GenerateWorldMacroEvent`

**Returns:** `Result<WorldMacroData, Error>`

  - `get_seed() -> u64`
  - `get_method() -> &String`
  - `get_settings() -> &Option<serde_yaml::Value>`

## `ChunkGenerateEvent`

**Returns:** `Result<ChunkData, Error>`

  - `get_chunk_position() -> &ChunkPosition`
  - `get_world_settings() -> &WorldGeneratorSettings`

## `ClientScriptEvent`

**Returns:** `Result<(), Error>`

  - `get_script_slug() -> &String`
  - `get_slug() -> &String`
  - `get_json() -> &String`
  - `get_player() -> Player`

## `PlayerSpawnEvent`

  - `get_player() -> Player`


# Managers

## `WorldsManager`

Global manager for all worlds. It provides access to a specific `WorldManager` by `world_slug`.

- `get_world_manager(world_slug: &String) -> WorldManager`

**Example:**
```rust
let worlds_manager = WorldsManager::singleton();
let world_manager = worlds_manager.get_world_manager(&world_slug)?;
let chunks_map = world_manager.get_chunks_map();
chunks_map.edit_block(payload.position, Some(payload.new_block_info))?;
```

## `WorldManager`

Manager for a single world. It stores the world slug and exposes that world’s `ChunksMap`.

- `create(slug: String) -> Self`
- `get_slug() -> &String`
- `get_chunks_map() -> ChunksMap`

## `ChunksMap`

Stores all per-chunk world data, including blocks and inventories, and provides access to modify it.

- `create(world_slug: String) -> Self`
- `edit_block(position: BlockPosition, new_block_info: Option<BlockDataInfo>) -> Result<(), Error>`
- `get_or_create_inventory(position: BlockPosition, slots_count: usize) -> Result<Inventory, Error>`

## `ItemsManager`

Global manager for custom items.

- `singleton() -> &'static Self`
- `add_item(item: ItemInfo) -> Result<(), Error>`

## `Plugin`

Plugin-local filesystem access. Paths are relative to the plugin root directory.

- `singleton() -> &'static Self`
- `read_dir(path: impl Into<String>) -> Vec<String>`
- `read_file(path: impl Into<String>) -> String`

**Examples:**
```rust
let plugin = Plugin::singleton();
let root_entries = plugin.read_dir("");
let items_entries = plugin.read_dir("items");
let weapons_yaml = plugin.read_file("items/weapons.yml");
```


# Server data

## `Player`

- `get_client_id() -> u64`
- `get_world_slug() -> Option<String>`
- `get_inventory() -> Inventory`
- `open_inventory(inventory: Inventory) -> Result<(), Error>`

## `Inventory`

- `get_id() -> u64`
- `add_item(item: Item) -> Result<(), AddItemError>`

`AddItemError` - `Full` / `NotFound`

**Example:**
```rust
match inventory.add_item(Item::create("test_armor")) {
    Ok(()) => {}
    Err(AddItemError::Full) => extism_pdk::log!(extism_pdk::LogLevel::Error, "Inventory is full"),
    Err(AddItemError::NotFound) => extism_pdk::log!(extism_pdk::LogLevel::Error, "Item not found"),
}
```

## `Item`

- `create(item_kind: impl Into<ItemKind>) -> Self`
- `amount(amount: u16) -> Self`

**Examples:**
```rust
let custom = Item::create("test_armor").amount(1);
let block = Item::create(block_index).amount(1);
```

## `ItemKind`

- `Block(BlockIndexType)`
- `CustomItem(String)`

**Custom item:**
```rust
let item = Item::create("test_weapon");
```

**Block item:**
```rust
let item = Item::create(block_index);
```

## `ItemInfo`

- `create(slug: impl Into<String>, item_type: ItemType, title: impl Into<String>, description: impl Into<String>) -> Self`

## `ItemType`

- `armor(body_part: BodyPart, icon: impl Into<String>, model: impl Into<String>) -> Self`
- `weapon(weapon_kind: WeaponKind, icon: impl Into<String>, model: impl Into<String>) -> Self`
