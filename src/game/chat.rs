use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::game::settings::{SettingsState, is_open as settings_open};
use crate::game::sky::LightingState;

pub struct ChatPlugin;

#[derive(Resource, Default)]
pub struct ChatState {
    pub open: bool,
    input: String,
    message: String,
}

#[derive(Component)]
struct ChatPanel;

#[derive(Component)]
struct ChatText;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChatState::default())
            .add_systems(Startup, spawn_chat)
            .add_systems(Update, (handle_chat_keys, update_chat_ui));
    }
}

pub fn is_open(state: &ChatState) -> bool {
    state.open
}

fn spawn_chat(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(18),
                right: px(18),
                bottom: px(88),
                min_height: px(34),
                padding: UiRect::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.54)),
            Visibility::Hidden,
            GlobalZIndex(i32::MAX - 3),
            ChatPanel,
        ))
        .with_child((
            Text::new(""),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
            ChatText,
        ));
}

fn handle_chat_keys(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<SettingsState>,
    mut state: ResMut<ChatState>,
    mut lighting: ResMut<LightingState>,
    mut cursor: Single<&mut CursorOptions>,
) {
    if !state.open {
        if settings_open(&settings) {
            return;
        }
        if keys.just_pressed(KeyCode::KeyT) {
            open_chat(&mut state, &mut cursor);
        } else if keys.just_pressed(KeyCode::Slash) {
            open_chat(&mut state, &mut cursor);
            state.input.push('/');
        }
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        close_chat(&mut state, &mut cursor);
        return;
    }

    if keys.just_pressed(KeyCode::Enter) {
        let input = state.input.trim().to_string();
        if !input.is_empty() {
            state.message = run_command(&input, &mut lighting);
        }
        close_chat(&mut state, &mut cursor);
        return;
    }

    if keys.just_pressed(KeyCode::Backspace) {
        state.input.pop();
    }

    if state.input.len() < 64 {
        for key in keys.get_just_pressed() {
            if let Some(character) = key_char(*key, keys.pressed(KeyCode::ShiftLeft)) {
                state.input.push(character);
            }
        }
    }
}

fn open_chat(state: &mut ChatState, cursor: &mut CursorOptions) {
    state.open = true;
    state.input.clear();
    cursor.visible = true;
    cursor.grab_mode = CursorGrabMode::None;
}

fn close_chat(state: &mut ChatState, cursor: &mut CursorOptions) {
    state.open = false;
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

fn run_command(input: &str, lighting: &mut LightingState) -> String {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() != 3 || parts[0] != "/time" || parts[1] != "set" {
        return "Unknown command".to_string();
    }

    match parts[2] {
        "day" => {
            lighting.set_clock(8, 0);
            "Time set to day".to_string()
        }
        "noon" => {
            lighting.set_clock(12, 0);
            "Time set to noon".to_string()
        }
        "night" => {
            lighting.set_clock(20, 0);
            "Time set to night".to_string()
        }
        "midnight" => {
            lighting.set_clock(0, 0);
            "Time set to midnight".to_string()
        }
        value => parse_clock(value)
            .map(|(hours, minutes)| {
                lighting.set_clock(hours, minutes);
                format!("Time set to {:02}:{:02}", hours, minutes)
            })
            .unwrap_or_else(|| "Bad time".to_string()),
    }
}

fn parse_clock(value: &str) -> Option<(u32, u32)> {
    if value.len() != 4 || !value.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let hours = value[0..2].parse::<u32>().ok()?;
    let minutes = value[2..4].parse::<u32>().ok()?;

    (hours < 24 && minutes < 60).then_some((hours, minutes))
}

fn key_char(key: KeyCode, shift: bool) -> Option<char> {
    let character = match key {
        KeyCode::KeyA => 'a',
        KeyCode::KeyB => 'b',
        KeyCode::KeyC => 'c',
        KeyCode::KeyD => 'd',
        KeyCode::KeyE => 'e',
        KeyCode::KeyF => 'f',
        KeyCode::KeyG => 'g',
        KeyCode::KeyH => 'h',
        KeyCode::KeyI => 'i',
        KeyCode::KeyJ => 'j',
        KeyCode::KeyK => 'k',
        KeyCode::KeyL => 'l',
        KeyCode::KeyM => 'm',
        KeyCode::KeyN => 'n',
        KeyCode::KeyO => 'o',
        KeyCode::KeyP => 'p',
        KeyCode::KeyQ => 'q',
        KeyCode::KeyR => 'r',
        KeyCode::KeyS => 's',
        KeyCode::KeyT => 't',
        KeyCode::KeyU => 'u',
        KeyCode::KeyV => 'v',
        KeyCode::KeyW => 'w',
        KeyCode::KeyX => 'x',
        KeyCode::KeyY => 'y',
        KeyCode::KeyZ => 'z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Space => ' ',
        KeyCode::Slash => '/',
        _ => return None,
    };

    if shift && character.is_ascii_alphabetic() {
        Some(character.to_ascii_uppercase())
    } else {
        Some(character)
    }
}

fn update_chat_ui(
    state: Res<ChatState>,
    mut panels: Query<&mut Visibility, With<ChatPanel>>,
    mut texts: Query<&mut Text, With<ChatText>>,
) {
    for mut panel in &mut panels {
        *panel = if state.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let Ok(mut text) = texts.single_mut() else {
        return;
    };

    text.0 = if state.open {
        format!("> {}", state.input)
    } else {
        state.message.clone()
    };
}
