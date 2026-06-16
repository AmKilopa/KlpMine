use bevy::{
    app::AppExit,
    color::palettes::css::WHITE,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::game::chat::{ChatState, is_open as chat_open};

pub struct SettingsPlugin;

#[derive(Resource)]
pub struct GameSettings {
    pub mouse_sensitivity: f32,
    pub fov: f32,
    pub render_distance: i32,
    pub bloom: bool,
    pub motion_blur: bool,
    pub color_grading: bool,
    pub fog: bool,
    pub shadows: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub ambience_volume: f32,
    pub effects_volume: f32,
}

#[derive(Resource)]
pub struct SettingsState {
    pub visible: bool,
}

#[derive(Component)]
struct SettingsPanel;

#[derive(Component)]
struct SettingsValue(SettingsKind);

#[derive(Component)]
struct SettingsButton {
    kind: SettingsKind,
    direction: f32,
}

#[derive(Component)]
struct ContinueButton;

#[derive(Component)]
struct ExitButton;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsKind {
    Mouse,
    Fov,
    RenderDistance,
    Bloom,
    MotionBlur,
    ColorGrading,
    Fog,
    Shadows,
    MasterVolume,
    MusicVolume,
    AmbienceVolume,
    EffectsVolume,
}

const MIN_MOUSE: f32 = 0.0009;
const MAX_MOUSE: f32 = 0.006;
const MIN_FOV: f32 = 65.0;
const MAX_FOV: f32 = 105.0;
const MIN_RENDER_DISTANCE: i32 = 2;
const MAX_RENDER_DISTANCE: i32 = 7;
const VOLUME_STEP: f32 = 0.1;

type SettingsButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<Button>),
>;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameSettings {
            mouse_sensitivity: 0.0025,
            fov: 85.0,
            render_distance: 3,
            bloom: true,
            motion_blur: true,
            color_grading: true,
            fog: true,
            shadows: false,
            master_volume: 0.8,
            music_volume: 0.55,
            ambience_volume: 0.7,
            effects_volume: 0.85,
        })
        .insert_resource(SettingsState { visible: false })
        .add_systems(Startup, spawn_settings_menu)
        .add_systems(
            Update,
            (
                toggle_settings_menu,
                handle_continue_button,
                handle_exit_button,
                handle_setting_buttons,
                refresh_button_visuals,
                refresh_settings_menu,
            ),
        );
    }
}

pub fn is_open(state: &SettingsState) -> bool {
    state.visible
}

fn spawn_settings_menu(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(i32::MAX - 4),
            SettingsPanel,
        ))
        .with_children(|root| {
            root.spawn((Node {
                width: px(360),
                padding: UiRect::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                ..default()
            },))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Settings"),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::from(WHITE)),
                    ));

                    setting_row(panel, "Mouse", SettingsKind::Mouse);
                    setting_row(panel, "FOV", SettingsKind::Fov);
                    setting_row(panel, "Chunks", SettingsKind::RenderDistance);
                    setting_row(panel, "Bloom", SettingsKind::Bloom);
                    setting_row(panel, "Motion blur", SettingsKind::MotionBlur);
                    setting_row(panel, "Color", SettingsKind::ColorGrading);
                    setting_row(panel, "Fog", SettingsKind::Fog);
                    setting_row(panel, "Shadows", SettingsKind::Shadows);
                    setting_row(panel, "Master", SettingsKind::MasterVolume);
                    setting_row(panel, "Music", SettingsKind::MusicVolume);
                    setting_row(panel, "Ambience", SettingsKind::AmbienceVolume);
                    setting_row(panel, "Effects", SettingsKind::EffectsVolume);

                    panel
                        .spawn((Node {
                            height: px(32),
                            margin: UiRect::top(px(2)),
                            column_gap: px(8),
                            ..default()
                        },))
                        .with_children(|buttons| {
                            menu_button(buttons, "Done", ContinueButton);
                            menu_button(buttons, "Exit", ExitButton);
                        });
                });
        });
}

fn menu_button(parent: &mut ChildSpawnerCommands, label: &'static str, marker: impl Component) {
    parent
        .spawn((
            Button,
            Node {
                width: px(96),
                height: px(32),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgba(0.8, 0.8, 0.8, 0.7)),
            marker,
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::from(WHITE)),
        ));
}

fn setting_row(parent: &mut ChildSpawnerCommands, title: &'static str, kind: SettingsKind) {
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            ..default()
        },))
        .with_children(|row| {
            row.spawn((Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|line| {
                    line.spawn((
                        Text::new(title),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.92, 0.92, 0.92)),
                    ));
                });

            row.spawn((Node {
                height: px(30),
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|controls| {
                    step_button(controls, kind, -1.0, "-");
                    controls
                        .spawn((Node {
                            width: px(88),
                            height: px(30),
                            flex_grow: 1.0,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },))
                        .with_child((
                            Text::new(""),
                            TextFont {
                                font_size: 17.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.92, 0.92)),
                            SettingsValue(kind),
                        ));
                    step_button(controls, kind, 1.0, "+");
                });
        });
}

fn step_button(
    parent: &mut ChildSpawnerCommands,
    kind: SettingsKind,
    direction: f32,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            Node {
                width: px(34),
                height: px(30),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgba(0.8, 0.8, 0.8, 0.7)),
            SettingsButton { kind, direction },
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(Color::from(WHITE)),
        ));
}

fn toggle_settings_menu(
    keys: Res<ButtonInput<KeyCode>>,
    chat_state: Res<ChatState>,
    mut state: ResMut<SettingsState>,
    mut cursor_options: Single<&mut CursorOptions>,
    mut panels: Query<&mut Visibility, With<SettingsPanel>>,
) {
    if chat_open(&chat_state) || !keys.just_pressed(KeyCode::Escape) {
        return;
    }

    set_settings_visible(!state.visible, &mut state, &mut cursor_options, &mut panels);
}

fn handle_continue_button(
    mut state: ResMut<SettingsState>,
    mut cursor_options: Single<&mut CursorOptions>,
    mut panels: Query<&mut Visibility, With<SettingsPanel>>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            set_settings_visible(false, &mut state, &mut cursor_options, &mut panels);
        }
    }
}

fn handle_exit_button(
    buttons: Query<&Interaction, (Changed<Interaction>, With<ExitButton>)>,
    mut exit: MessageWriter<AppExit>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            exit.write(AppExit::Success);
        }
    }
}

fn set_settings_visible(
    visible: bool,
    state: &mut SettingsState,
    cursor_options: &mut CursorOptions,
    panels: &mut Query<&mut Visibility, With<SettingsPanel>>,
) {
    state.visible = visible;
    cursor_options.visible = visible;
    cursor_options.grab_mode = if visible {
        CursorGrabMode::None
    } else {
        CursorGrabMode::Locked
    };

    for mut panel in panels {
        *panel = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn handle_setting_buttons(
    mut settings: ResMut<GameSettings>,
    buttons: Query<(&Interaction, &SettingsButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match button.kind {
            SettingsKind::Mouse => {
                settings.mouse_sensitivity = (settings.mouse_sensitivity
                    + button.direction * 0.0002)
                    .clamp(MIN_MOUSE, MAX_MOUSE);
            }
            SettingsKind::Fov => {
                settings.fov = (settings.fov + button.direction * 2.5).clamp(MIN_FOV, MAX_FOV);
            }
            SettingsKind::RenderDistance => {
                settings.render_distance = (settings.render_distance + button.direction as i32)
                    .clamp(MIN_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
            }
            SettingsKind::Bloom => {
                if button.direction != 0.0 {
                    settings.bloom = !settings.bloom;
                }
            }
            SettingsKind::MotionBlur => {
                if button.direction != 0.0 {
                    settings.motion_blur = !settings.motion_blur;
                }
            }
            SettingsKind::ColorGrading => {
                if button.direction != 0.0 {
                    settings.color_grading = !settings.color_grading;
                }
            }
            SettingsKind::Fog => {
                if button.direction != 0.0 {
                    settings.fog = !settings.fog;
                }
            }
            SettingsKind::Shadows => {
                if button.direction != 0.0 {
                    settings.shadows = !settings.shadows;
                }
            }
            SettingsKind::MasterVolume => {
                settings.master_volume =
                    (settings.master_volume + button.direction * VOLUME_STEP).clamp(0.0, 1.0);
            }
            SettingsKind::MusicVolume => {
                settings.music_volume =
                    (settings.music_volume + button.direction * VOLUME_STEP).clamp(0.0, 1.0);
            }
            SettingsKind::AmbienceVolume => {
                settings.ambience_volume =
                    (settings.ambience_volume + button.direction * VOLUME_STEP).clamp(0.0, 1.0);
            }
            SettingsKind::EffectsVolume => {
                settings.effects_volume =
                    (settings.effects_volume + button.direction * VOLUME_STEP).clamp(0.0, 1.0);
            }
        }
    }
}

fn refresh_settings_menu(
    settings: Res<GameSettings>,
    mut values: Query<(&SettingsValue, &mut Text)>,
) {
    for (value, mut text) in &mut values {
        text.0 = match value.0 {
            SettingsKind::Mouse => format!("{:.1}", settings.mouse_sensitivity * 1000.0),
            SettingsKind::Fov => format!("{:.0}", settings.fov),
            SettingsKind::RenderDistance => settings.render_distance.to_string(),
            SettingsKind::Bloom => on_off(settings.bloom),
            SettingsKind::MotionBlur => on_off(settings.motion_blur),
            SettingsKind::ColorGrading => on_off(settings.color_grading),
            SettingsKind::Fog => on_off(settings.fog),
            SettingsKind::Shadows => on_off(settings.shadows),
            SettingsKind::MasterVolume => percent(settings.master_volume),
            SettingsKind::MusicVolume => percent(settings.music_volume),
            SettingsKind::AmbienceVolume => percent(settings.ambience_volume),
            SettingsKind::EffectsVolume => percent(settings.effects_volume),
        };
    }
}

fn on_off(value: bool) -> String {
    if value {
        "on".to_string()
    } else {
        "off".to_string()
    }
}

fn percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

fn refresh_button_visuals(mut buttons: SettingsButtonQuery) {
    for (interaction, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::Pressed => BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
            Interaction::Hovered => BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
            Interaction::None => BackgroundColor(Color::NONE),
        };
    }
}
