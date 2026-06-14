use bevy::prelude::*;

use crate::game::events::PlayerDamaged;

pub struct HealthPlugin;

#[derive(Resource)]
pub struct PlayerHealth {
    pub current: f32,
    pub max: f32,
}

#[derive(Component)]
struct HealthText;

#[derive(Component)]
struct DamageVignette;

#[derive(Resource, Default)]
struct DamageFlash {
    amount: f32,
}

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlayerHealth {
            current: 10.0,
            max: 10.0,
        })
        .insert_resource(DamageFlash::default())
        .add_systems(Startup, (spawn_health_ui, spawn_damage_vignette))
        .add_systems(
            Update,
            (apply_damage, update_health_ui, update_damage_vignette),
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
) {
    for event in damage.read() {
        health.current = (health.current - event.amount).clamp(0.0, health.max);
        flash.amount = (flash.amount + 0.18 + event.amount * 0.08).clamp(0.0, 0.55);
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
    flash.amount = (flash.amount - time.delta_secs() * 1.35).max(0.0);

    for mut color in &mut overlays {
        *color = BackgroundColor(Color::srgba(0.85, 0.0, 0.0, flash.amount));
    }
}
