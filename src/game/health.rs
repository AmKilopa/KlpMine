use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::game::events::{PlayerDamaged, PlayerDied, PlayerRespawned};

pub struct HealthPlugin;

#[derive(Resource)]
pub struct PlayerHealth {
    pub current: f32,
    pub max: f32,
    pub dead: bool,
}

#[derive(Component)]
struct HealthText;

#[derive(Component)]
struct DamageVignette;

#[derive(Component)]
struct DeathScreen;

#[derive(Component)]
struct RespawnButton;

#[derive(Resource, Default)]
struct DamageFlash {
    current: f32,
    target: f32,
}

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlayerHealth {
            current: 10.0,
            max: 10.0,
            dead: false,
        })
        .insert_resource(DamageFlash::default())
        .add_systems(Startup, (spawn_health_ui, spawn_damage_vignette))
        .add_systems(
            Update,
            (
                apply_damage,
                update_health_ui,
                update_damage_vignette,
                show_death_screen,
                respawn_button,
            ),
        );
    }
}

fn spawn_health_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("HP 10.0"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(24),
            bottom: px(28),
            ..default()
        },
        GlobalZIndex(i32::MAX - 7),
        HealthText,
    ));
}

fn spawn_damage_vignette(mut commands: Commands) {
    let color = BackgroundColor(Color::srgba(0.85, 0.0, 0.0, 0.0));
    let z = GlobalZIndex(i32::MAX - 2);

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: px(86),
            ..default()
        },
        color,
        z,
        DamageVignette,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            bottom: px(0),
            width: percent(100),
            height: px(86),
            ..default()
        },
        color,
        z,
        DamageVignette,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(110),
            height: percent(100),
            ..default()
        },
        color,
        z,
        DamageVignette,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(0),
            width: px(110),
            height: percent(100),
            ..default()
        },
        color,
        z,
        DamageVignette,
    ));
}

fn apply_damage(
    mut health: ResMut<PlayerHealth>,
    mut flash: ResMut<DamageFlash>,
    mut damage: MessageReader<PlayerDamaged>,
    mut died: MessageWriter<PlayerDied>,
) {
    for event in damage.read() {
        if health.dead {
            continue;
        }

        health.current = (health.current - event.amount).clamp(0.0, health.max);
        flash.target = (flash.target + 0.22 + event.amount * 0.09).clamp(0.0, 0.72);

        if health.current <= 0.0 {
            health.dead = true;
            died.write(PlayerDied);
        }
    }
}

fn update_health_ui(health: Res<PlayerHealth>, mut texts: Query<&mut Text, With<HealthText>>) {
    if !health.is_changed() {
        return;
    }

    let Ok(mut text) = texts.single_mut() else {
        return;
    };

    text.0 = format!("HP {:.1}", health.current);
}

fn update_damage_vignette(
    time: Res<Time>,
    mut flash: ResMut<DamageFlash>,
    mut overlays: Query<&mut BackgroundColor, With<DamageVignette>>,
) {
    let dt = time.delta_secs();
    flash.target = (flash.target - dt * 0.95).max(0.0);
    flash.current += (flash.target - flash.current) * (1.0 - (-8.0 * dt).exp());
    let alpha = flash.current * flash.current;

    for mut color in &mut overlays {
        *color = BackgroundColor(Color::srgba(0.8, 0.0, 0.0, alpha));
    }
}

fn show_death_screen(
    mut commands: Commands,
    health: Res<PlayerHealth>,
    screens: Query<Entity, With<DeathScreen>>,
    mut cursor_options: Single<&mut CursorOptions>,
) {
    if health.dead {
        if screens.is_empty() {
            cursor_options.visible = true;
            cursor_options.grab_mode = CursorGrabMode::None;
            spawn_death_screen(&mut commands);
        }
    } else {
        for entity in &screens {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_death_screen(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: px(18),
                ..default()
            },
            BackgroundColor(Color::srgba(0.18, 0.0, 0.0, 0.48)),
            GlobalZIndex(i32::MAX - 1),
            DeathScreen,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("You died"),
                TextFont {
                    font_size: 42.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent
                .spawn((
                    Button,
                    Node {
                        width: px(180),
                        height: px(44),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.08, 0.08, 0.82)),
                    BorderColor::all(Color::srgba(0.8, 0.8, 0.8, 0.85)),
                    RespawnButton,
                ))
                .with_child((
                    Text::new("Respawn"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
        });
}

fn respawn_button(
    mut health: ResMut<PlayerHealth>,
    mut respawned: MessageWriter<PlayerRespawned>,
    mut flash: ResMut<DamageFlash>,
    mut buttons: Query<&Interaction, (Changed<Interaction>, With<RespawnButton>)>,
    mut cursor_options: Single<&mut CursorOptions>,
) {
    for interaction in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        health.current = health.max;
        health.dead = false;
        flash.current = 0.0;
        flash.target = 0.0;
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
        respawned.write(PlayerRespawned);
    }
}
