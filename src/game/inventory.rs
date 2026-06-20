use bevy::{
    input::mouse::AccumulatedMouseScroll,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::game::{
    chat::{ChatState, is_open as chat_open},
    events::UiFocus,
    settings::{SettingsState, is_open as settings_open},
    world::Block,
};

pub struct InventoryPlugin;

#[derive(Clone, Copy)]
pub struct ItemStack {
    pub block: Block,
    pub count: u32,
}

#[derive(Resource)]
pub struct Inventory {
    pub slots: [Option<ItemStack>; 36],
    pub selected: usize,
    pub is_open: bool,
}

#[derive(Resource)]
struct DragState {
    stack: Option<ItemStack>,
    from_slot: usize,
}

#[derive(Component)]
struct HotbarSlot(usize);

#[derive(Component)]
struct InventorySlot(usize);

#[derive(Component)]
struct HotbarIcon {
    slot: usize,
}

#[derive(Component)]
struct InventoryIcon {
    slot: usize,
}

#[derive(Component)]
struct SlotCount(usize);

#[derive(Component)]
struct InventoryPanel;

#[derive(Component)]
struct DragGhost;

#[derive(Component)]
struct InventoryOverlay;

#[derive(Component)]
struct GameHotbar;

#[derive(Resource, Clone)]
struct HotbarAtlas {
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        let image = app
            .world_mut()
            .resource_scope(|_world, server: Mut<AssetServer>| {
                server.load("textures/block_atlas.png")
            });
        let layout = app
            .world_mut()
            .resource_mut::<Assets<TextureAtlasLayout>>()
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(34),
                8,
                1,
                None,
                None,
            ));

        app.insert_resource(Inventory::new())
            .insert_resource(DragState {
                stack: None,
                from_slot: 0,
            })
            .insert_resource(HotbarAtlas { image, layout })
            .add_systems(Startup, (spawn_hotbar, spawn_inventory_panel, spawn_drag_ghost))
            .add_systems(
                Update,
                (
                    select_hotbar_slot,
                    update_hotbar_ui,
                    toggle_inventory,
                    handle_inventory_interaction,
                    update_inventory_ui,
                    update_drag_ghost,
                ),
            );
    }
}

impl Inventory {
    fn new() -> Self {
        Self {
            slots: [None; 36],
            selected: 0,
            is_open: false,
        }
    }

    pub fn add(&mut self, block: Block) -> bool {
        for slot in self.slots.iter_mut().flatten() {
            if slot.block == block && slot.count < 99 {
                slot.count += 1;
                return true;
            }
        }
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(ItemStack { block, count: 1 });
                return true;
            }
        }
        false
    }

    pub fn can_add(&self, block: Block) -> bool {
        for slot in self.slots.iter().flatten() {
            if slot.block == block && slot.count < 99 {
                return true;
            }
        }
        self.slots.iter().any(|slot| slot.is_none())
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.slots[self.selected].map(|s| s.block)
    }

    pub fn remove_selected(&mut self) -> Option<Block> {
        let slot = self.slots[self.selected].as_mut()?;
        let block = slot.block;
        slot.count = slot.count.saturating_sub(1);
        if slot.count == 0 {
            self.slots[self.selected] = None;
        }
        Some(block)
    }

    pub fn take_all(&mut self) -> Vec<ItemStack> {
        self.slots
            .iter_mut()
            .filter_map(|slot| slot.take())
            .collect()
    }

    #[allow(dead_code)]
    pub fn move_stack(&mut self, from: usize, to: usize) {
        if from >= 36 || to >= 36 {
            return;
        }
        let taken = self.slots[from].take();
        let target = self.slots[to].take();
        match (taken, target) {
            (None, None) => {}
            (Some(stack), None) => {
                self.slots[to] = Some(stack);
            }
            (Some(stack), Some(existing)) if existing.block == stack.block => {
                let total = existing.count + stack.count;
                if total <= 99 {
                    self.slots[to] = Some(ItemStack { block: stack.block, count: total });
                } else {
                    self.slots[to] = Some(ItemStack { block: stack.block, count: 99 });
                    self.slots[from] = Some(ItemStack { block: stack.block, count: total - 99 });
                }
            }
            (Some(stack), Some(existing)) => {
                self.slots[to] = Some(stack);
                self.slots[from] = Some(existing);
            }
            (None, Some(stack)) => {
                self.slots[from] = Some(stack);
                self.slots[to] = None;
            }
        }
    }

    fn return_drag(&mut self, block: Block, count: u32, from_slot: usize) {
        let mut remaining = count;
        for slot in self.slots.iter_mut().flatten() {
            if slot.block == block && slot.count < 99 {
                let space = 99 - slot.count;
                let add = remaining.min(space);
                slot.count += add;
                remaining -= add;
                if remaining == 0 {
                    return;
                }
            }
        }
        for slot in &mut self.slots {
            if slot.is_none() {
                let add = remaining.min(99);
                *slot = Some(ItemStack { block, count: add });
                remaining -= add;
                if remaining == 0 {
                    return;
                }
            }
        }
        if let Some(existing) = &mut self.slots[from_slot] {
            existing.count += remaining;
        } else {
            self.slots[from_slot] = Some(ItemStack { block, count: remaining });
        }
    }


}

fn spawn_hotbar(mut commands: Commands, atlas: Res<HotbarAtlas>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                bottom: px(24),
                width: px(432),
                height: px(42),
                margin: UiRect::left(px(-216)),
                column_gap: px(6),
                ..default()
            },
            GlobalZIndex(i32::MAX - 8),
            GameHotbar,
        ))
        .with_children(|bar| {
            for index in 0..9 {
                bar.spawn((
                    Node {
                        width: px(42),
                        height: px(42),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.04, 0.04, 0.42)),
                    BorderColor::all(Color::srgba(0.64, 0.64, 0.64, 0.7)),
                    HotbarSlot(index),
                    Button,
                ))
                .with_children(|slot| {
                    block_icon(slot, &atlas, index);
                })
                .with_child((
                    Text::new(""),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(3),
                        bottom: px(2),
                        ..default()
                    },
                    SlotCount(index),
                ));
            }
        });
}

fn spawn_inventory_panel(mut commands: Commands, atlas: Res<HotbarAtlas>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                right: px(0.0),
                top: px(0.0),
                bottom: px(0.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            InventoryOverlay,
            GlobalZIndex(i32::MAX - 20),
        ))
        .with_children(|overlay| {
            overlay.spawn((
                Text::new("Inventory"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(px(4.0)),
                    ..default()
                },
            ));

            overlay
                .spawn((
                    Node {
                        width: px(456),
                        height: px(204),
                        flex_wrap: FlexWrap::Wrap,
                        align_content: AlignContent::FlexStart,
                        column_gap: px(6),
                        row_gap: px(6),
                        ..default()
                    },
                    InventoryPanel,
                ))
                .with_children(|grid| {
                    for index in 9..36 {
                        grid.spawn((
                            Node {
                                width: px(42),
                                height: px(42),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(px(1)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.08, 0.08, 0.5)),
                            BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 0.5)),
                            Button,
                            InventorySlot(index),
                        ))
                        .with_children(|slot| {
                            block_icon_inv(slot, &atlas, index);
                        })
                        .with_child((
                            Text::new(""),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Node {
                                position_type: PositionType::Absolute,
                                right: px(3),
                                bottom: px(2),
                                ..default()
                            },
                            SlotCount(index),
                        ));
                    }
                });

            let sep = Node {
                width: px(456),
                height: px(2),
                margin: UiRect::vertical(px(4)),
                ..default()
            };
            overlay.spawn((sep, BackgroundColor(Color::srgba(0.5, 0.5, 0.5, 0.3))));

            overlay
                .spawn((Node {
                    width: px(456),
                    height: px(48),
                    flex_wrap: FlexWrap::Wrap,
                    align_content: AlignContent::FlexStart,
                    column_gap: px(6),
                    ..default()
                },))
                .with_children(|hotbar_row| {
                    for index in 0..9 {
                        hotbar_row
                            .spawn((
                                Node {
                                    width: px(42),
                                    height: px(42),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border: UiRect::all(px(1)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.04, 0.04, 0.04, 0.42)),
                                BorderColor::all(Color::srgba(0.64, 0.64, 0.64, 0.7)),
                                HotbarSlot(index),
                                Button,
                            ))
                            .with_children(|slot| {
                                block_icon_inv(slot, &atlas, index);
                            })
                            .with_child((
                                Text::new(""),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                Node {
                                    position_type: PositionType::Absolute,
                                    right: px(3),
                                    bottom: px(2),
                                    ..default()
                                },
                                SlotCount(index),
                            ));
                    }
                });
        });
}

fn block_icon(parent: &mut ChildSpawnerCommands, atlas: &HotbarAtlas, slot: usize) {
    parent.spawn((
        ImageNode::from_atlas_image(
            atlas.image.clone(),
            TextureAtlas {
                layout: atlas.layout.clone(),
                index: 0,
            },
        )
        .with_color(Color::NONE),
        Node {
            width: px(28),
            height: px(28),
            ..default()
        },
        HotbarIcon { slot },
    ));
}

fn block_icon_inv(parent: &mut ChildSpawnerCommands, atlas: &HotbarAtlas, slot: usize) {
    parent.spawn((
        ImageNode::from_atlas_image(
            atlas.image.clone(),
            TextureAtlas {
                layout: atlas.layout.clone(),
                index: 0,
            },
        )
        .with_color(Color::NONE),
        Node {
            width: px(28),
            height: px(28),
            ..default()
        },
        InventoryIcon { slot },
    ));
}

fn spawn_drag_ghost(mut commands: Commands, atlas: Res<HotbarAtlas>) {
    commands.spawn((
        ImageNode::from_atlas_image(
            atlas.image.clone(),
            TextureAtlas {
                layout: atlas.layout.clone(),
                index: 0,
            },
        )
        .with_color(Color::NONE),
        Node {
            position_type: PositionType::Absolute,
            width: px(34),
            height: px(34),
            ..default()
        },
        GlobalZIndex(i32::MAX),
        DragGhost,
    ));
}

fn update_drag_ghost(
    drag: Res<DragState>,
    windows: Query<&Window>,
    ghost: Single<(&mut ImageNode, &mut Node), With<DragGhost>>,
) {
    let (mut image, mut node) = ghost.into_inner();
    if let Some(stack) = &drag.stack {
        image.color = Color::WHITE;
        if let Some(atlas) = &mut image.texture_atlas {
            atlas.index = stack.block.atlas_index();
        }
        if let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) {
            node.left = Val::Px(cursor.x - 17.0);
            node.top = Val::Px(cursor.y - 17.0);
        }
    } else {
        image.color = Color::NONE;
    }
}

fn select_hotbar_slot(
    focus: Res<UiFocus>,
    scroll: Res<AccumulatedMouseScroll>,
    keys: Res<ButtonInput<KeyCode>>,
    mut inv: ResMut<Inventory>,
) {
    if !focus.cursor_locked() {
        return;
    }

    if scroll.delta.y > 0.0 {
        inv.selected = (inv.selected + 8) % 9;
    } else if scroll.delta.y < 0.0 {
        inv.selected = (inv.selected + 1) % 9;
    }

    const NUMBER_KEYS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];

    for (index, key) in NUMBER_KEYS.into_iter().enumerate() {
        if keys.just_pressed(key) {
            inv.selected = index;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    settings_state: Res<SettingsState>,
    chat_state: Res<ChatState>,
    mut inventory: ResMut<Inventory>,
    mut drag: ResMut<DragState>,
    mut focus: ResMut<UiFocus>,
    mut cursor: Single<&mut CursorOptions>,
    mut overlay: Query<&mut Node, (With<InventoryOverlay>, Without<GameHotbar>)>,
    mut hotbar: Query<&mut Node, (With<GameHotbar>, Without<InventoryOverlay>)>,
) {
    if settings_open(&settings_state) || chat_open(&chat_state) {
        return;
    }

    if keys.just_pressed(KeyCode::KeyE) || (inventory.is_open && keys.just_pressed(KeyCode::Escape))
    {
        inventory.is_open = !inventory.is_open;
        overlay.single_mut().unwrap().display = if inventory.is_open {
            Display::Flex
        } else {
            Display::None
        };
        hotbar.single_mut().unwrap().display = if inventory.is_open {
            Display::None
        } else {
            Display::Flex
        };
        focus.inventory_open = inventory.is_open;
        if inventory.is_open {
            cursor.visible = true;
            cursor.grab_mode = CursorGrabMode::None;
        } else {
            cursor.visible = false;
            cursor.grab_mode = CursorGrabMode::Locked;
            if let Some(stack) = drag.stack.take() {
                inventory.return_drag(stack.block, stack.count, drag.from_slot);
            }
        }
    }
}

fn handle_slot_click(drag: &mut DragState, inv: &mut Inventory, target: usize) {
    let Some(stack) = drag.stack else {
        if inv.slots[target].is_some() {
            drag.stack = inv.slots[target];
            drag.from_slot = target;
            inv.slots[target] = None;
        }
        return;
    };

    let taken = inv.slots[target].take();
    match taken {
        None => {
            inv.slots[target] = Some(stack);
        }
        Some(existing) if existing.block == stack.block => {
            let total = existing.count + stack.count;
            if total <= 99 {
                inv.slots[target] = Some(ItemStack { block: stack.block, count: total });
            } else {
                inv.slots[target] = Some(ItemStack { block: stack.block, count: 99 });
                inv.slots[drag.from_slot] = Some(ItemStack { block: stack.block, count: total - 99 });
            }
        }
        Some(existing) => {
            inv.slots[target] = Some(stack);
            inv.slots[drag.from_slot] = Some(existing);
        }
    }
    drag.stack = None;
}

fn handle_inventory_interaction(
    mut drag: ResMut<DragState>,
    mut inv: ResMut<Inventory>,
    hotbar_slots: Query<(&Interaction, &HotbarSlot), Changed<Interaction>>,
    inv_slots: Query<(&Interaction, &InventorySlot), Changed<Interaction>>,
) {
    if !inv.is_open {
        return;
    }

    for (interaction, slot) in &hotbar_slots {
        if *interaction != Interaction::Pressed {
            continue;
        }
        handle_slot_click(&mut drag, &mut inv, slot.0);
    }

    for (interaction, slot) in &inv_slots {
        if *interaction != Interaction::Pressed {
            continue;
        }
        handle_slot_click(&mut drag, &mut inv, slot.0);
    }
}

#[allow(clippy::type_complexity)]
fn update_hotbar_ui(
    inventory: Res<Inventory>,
    mut slots: Query<(&HotbarSlot, &mut BorderColor, &mut BackgroundColor)>,
    mut icons: Query<(&HotbarIcon, &mut ImageNode)>,
    mut counts: Query<(&SlotCount, &mut Text), (With<HotbarSlot>, Without<InventoryPanel>)>,
) {
    for (slot, mut border, mut background) in &mut slots {
        if slot.0 == inventory.selected {
            *border = BorderColor::all(Color::WHITE);
            *background = BackgroundColor(Color::srgba(0.16, 0.16, 0.16, 0.62));
        } else {
            *border = BorderColor::all(Color::srgba(0.64, 0.64, 0.64, 0.7));
            *background = BackgroundColor(Color::srgba(0.04, 0.04, 0.04, 0.42));
        }
    }

    for (icon, mut image) in &mut icons {
        if let Some(stack) = inventory.slots[icon.slot] {
            image.color = Color::WHITE;
            if let Some(atlas) = &mut image.texture_atlas {
                atlas.index = stack.block.atlas_index();
            }
        } else {
            image.color = Color::NONE;
        }
    }

    for (count_slot, mut text) in &mut counts {
        text.0 = inventory.slots[count_slot.0]
            .map(|s| {
                if s.count > 1 {
                    s.count.to_string()
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
    }
}

fn update_inventory_ui(
    inventory: Res<Inventory>,
    mut icons: Query<(&InventoryIcon, &mut ImageNode)>,
    mut counts: Query<(&SlotCount, &mut Text), Without<HotbarSlot>>,
) {
    for (icon, mut image) in &mut icons {
        if let Some(stack) = inventory.slots[icon.slot] {
            image.color = Color::WHITE;
            if let Some(atlas) = &mut image.texture_atlas {
                atlas.index = stack.block.atlas_index();
            }
        } else {
            image.color = Color::NONE;
        }
    }

    for (count_slot, mut text) in &mut counts {
        text.0 = inventory.slots[count_slot.0]
            .map(|s| {
                if s.count > 1 {
                    s.count.to_string()
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_single_item_occupies_slot() {
        let mut inv = Inventory::new();
        assert!(inv.add(Block::Dirt));
        assert_eq!(inv.slots[0].unwrap().block, Block::Dirt);
        assert_eq!(inv.slots[0].unwrap().count, 1);
    }

    #[test]
    fn add_to_existing_stack_increments_count() {
        let mut inv = Inventory::new();
        inv.add(Block::Dirt);
        inv.add(Block::Dirt);
        assert_eq!(inv.slots[0].unwrap().count, 2);
    }

    #[test]
    fn add_different_items_uses_different_slots() {
        let mut inv = Inventory::new();
        inv.add(Block::Dirt);
        inv.add(Block::Stone);
        assert_eq!(inv.slots[0].unwrap().block, Block::Dirt);
        assert_eq!(inv.slots[1].unwrap().block, Block::Stone);
    }

    #[test]
    fn add_does_not_exceed_99() {
        let mut inv = Inventory::new();
        for _ in 0..100 {
            inv.add(Block::Dirt);
        }
        assert_eq!(inv.slots[0].unwrap().count, 99);
        assert_eq!(inv.slots[1].unwrap().count, 1);
    }

    #[test]
    fn remove_selected_decrements_count() {
        let mut inv = Inventory::new();
        inv.add(Block::Dirt);
        inv.add(Block::Dirt);
        assert_eq!(inv.remove_selected(), Some(Block::Dirt));
        assert_eq!(inv.slots[0].unwrap().count, 1);
    }

    #[test]
    fn remove_selected_empties_slot_at_zero() {
        let mut inv = Inventory::new();
        inv.add(Block::Dirt);
        assert_eq!(inv.remove_selected(), Some(Block::Dirt));
        assert!(inv.slots[0].is_none());
    }

    #[test]
    fn selected_block_returns_none_for_empty_slot() {
        let inv = Inventory::new();
        assert!(inv.selected_block().is_none());
    }

    #[test]
    fn take_all_empties_inventory() {
        let mut inv = Inventory::new();
        inv.add(Block::Dirt);
        inv.add(Block::Stone);
        let stacks = inv.take_all();
        assert_eq!(stacks.len(), 2);
        assert!(inv.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn can_add_returns_true_when_space_available() {
        let mut inv = Inventory::new();
        assert!(inv.can_add(Block::Dirt));
        inv.add(Block::Dirt);
        assert!(inv.can_add(Block::Dirt));
    }

    #[test]
    fn move_stack_swaps_different_blocks() {
        let mut inv = Inventory::new();
        inv.add(Block::Dirt);
        inv.add(Block::Stone);
        inv.move_stack(0, 1);
        assert_eq!(inv.slots[0].unwrap().block, Block::Stone);
        assert_eq!(inv.slots[1].unwrap().block, Block::Dirt);
    }

    #[test]
    fn move_stack_merges_same_block() {
        let mut inv = Inventory::new();
        inv.add(Block::Dirt);
        inv.add(Block::Dirt);
        assert_eq!(inv.slots[0].unwrap().count, 2);
        assert!(inv.slots[1].is_none());
    }
}
