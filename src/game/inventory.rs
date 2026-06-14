use bevy::{input::mouse::AccumulatedMouseScroll, prelude::*};

use crate::game::{
    settings::{SettingsState, is_open},
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
    pub slots: [Option<ItemStack>; 9],
    pub selected: usize,
}

#[derive(Component)]
struct HotbarSlot(usize);

#[derive(Component)]
struct HotbarIcon(usize);

#[derive(Component)]
struct HotbarCount(usize);

#[derive(Resource)]
struct HotbarAtlas {
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Inventory::new())
            .add_systems(Startup, (setup_hotbar_atlas, spawn_hotbar).chain())
            .add_systems(Update, (select_hotbar_slot, update_hotbar));
    }
}

impl Inventory {
    fn new() -> Self {
        Self {
            slots: [None; 9],
            selected: 0,
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

    pub fn selected_block(&self) -> Option<Block> {
        self.slots[self.selected].map(|slot| slot.block)
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
}

fn setup_hotbar_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.insert_resource(HotbarAtlas {
        image: asset_server.load("textures/block_atlas.png"),
        layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(34),
            3,
            1,
            None,
            None,
        )),
    });
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
                ))
                .with_child((
                    ImageNode::from_atlas_image(
                        atlas.image.clone(),
                        TextureAtlas {
                            layout: atlas.layout.clone(),
                            index: 0,
                        },
                    )
                    .with_color(Color::NONE),
                    Node {
                        width: px(26),
                        height: px(26),
                        ..default()
                    },
                    HotbarIcon(index),
                ))
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
                    HotbarCount(index),
                ));
            }
        });
}

fn select_hotbar_slot(
    settings_state: Res<SettingsState>,
    scroll: Res<AccumulatedMouseScroll>,
    keys: Res<ButtonInput<KeyCode>>,
    mut inventory: ResMut<Inventory>,
) {
    if is_open(&settings_state) {
        return;
    }

    if scroll.delta.y > 0.0 {
        inventory.selected = (inventory.selected + 8) % 9;
    } else if scroll.delta.y < 0.0 {
        inventory.selected = (inventory.selected + 1) % 9;
    }

    let number_keys = [
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

    for (index, key) in number_keys.into_iter().enumerate() {
        if keys.just_pressed(key) {
            inventory.selected = index;
        }
    }
}

fn update_hotbar(
    inventory: Res<Inventory>,
    mut slots: Query<(&HotbarSlot, &mut BorderColor, &mut BackgroundColor)>,
    mut icons: Query<(&HotbarIcon, &mut ImageNode)>,
    mut counts: Query<(&HotbarCount, &mut Text)>,
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

    for (icon_slot, mut image) in &mut icons {
        if let Some(slot) = inventory.slots[icon_slot.0] {
            image.color = Color::WHITE;
            if let Some(atlas) = &mut image.texture_atlas {
                atlas.index = slot.block.hotbar_tile();
            }
        } else {
            image.color = Color::NONE;
        }
    }

    for (count_slot, mut text) in &mut counts {
        text.0 = inventory.slots[count_slot.0]
            .map(|slot| slot.count.to_string())
            .unwrap_or_default();
    }
}
