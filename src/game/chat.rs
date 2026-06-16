use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::game::settings::{SettingsState, is_open as settings_open};
use crate::game::sky::LightingState;
use crate::game::world::WorldSeed;

pub struct ChatPlugin;

#[derive(Resource, Default)]
pub struct ChatState {
    pub open: bool,
    input: String,
    history: Vec<String>,
    suggestions: Vec<CommandSuggestion>,
    suggestion_index: usize,
    message_timer: f32,
}

#[derive(Component)]
struct ChatPanel;

#[derive(Component)]
struct ChatText;

#[derive(Clone)]
struct CommandSuggestion {
    insert: &'static str,
    help: &'static str,
}

struct CommandInfo {
    insert: &'static str,
    help: &'static str,
}

const COMMANDS: [CommandInfo; 6] = [
    CommandInfo {
        insert: "/help",
        help: "show commands",
    },
    CommandInfo {
        insert: "/seed",
        help: "print current world seed",
    },
    CommandInfo {
        insert: "/time set day",
        help: "set time to 08:00",
    },
    CommandInfo {
        insert: "/time set noon",
        help: "set time to 12:00",
    },
    CommandInfo {
        insert: "/time set night",
        help: "set time to 20:00",
    },
    CommandInfo {
        insert: "/time set midnight",
        help: "set time to 00:00",
    },
];

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
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<SettingsState>,
    mut state: ResMut<ChatState>,
    mut lighting: ResMut<LightingState>,
    seed: Res<WorldSeed>,
    mut cursor: Single<&mut CursorOptions>,
) {
    state.message_timer = (state.message_timer - time.delta_secs()).max(0.0);

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
            push_history(&mut state, format!("> {}", input));
            let result = run_command(&input, &mut lighting, seed.value);
            push_history(&mut state, result);
            state.message_timer = 6.0;
        }
        close_chat(&mut state, &mut cursor);
        return;
    }

    if keys.just_pressed(KeyCode::Backspace) {
        state.input.pop();
        refresh_suggestions(&mut state);
        return;
    }

    if keys.just_pressed(KeyCode::ArrowUp) {
        select_suggestion(&mut state, -1);
        return;
    }

    if keys.just_pressed(KeyCode::ArrowDown) {
        select_suggestion(&mut state, 1);
        return;
    }

    if keys.just_pressed(KeyCode::Tab) {
        accept_suggestion(&mut state);
        return;
    }

    if state.input.len() < 64 {
        let mut changed = false;
        for key in keys.get_just_pressed() {
            if let Some(character) = key_char(*key, keys.pressed(KeyCode::ShiftLeft)) {
                state.input.push(character);
                changed = true;
            }
        }
        if changed {
            refresh_suggestions(&mut state);
        }
    }
}

fn open_chat(state: &mut ChatState, cursor: &mut CursorOptions) {
    state.open = true;
    state.input.clear();
    state.suggestions.clear();
    state.suggestion_index = 0;
    cursor.visible = true;
    cursor.grab_mode = CursorGrabMode::None;
}

fn close_chat(state: &mut ChatState, cursor: &mut CursorOptions) {
    state.open = false;
    state.suggestions.clear();
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

fn run_command(input: &str, lighting: &mut LightingState, seed: u64) -> String {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() == 1 && parts[0] == "/help" {
        return COMMANDS
            .iter()
            .map(|command| format!("{} - {}", command.insert, command.help))
            .collect::<Vec<_>>()
            .join("\n");
    }

    if parts.len() == 1 && parts[0] == "/seed" {
        return format!("Seed: {}", seed);
    }

    if parts.len() != 3 || parts[0] != "/time" || parts[1] != "set" {
        return unknown_command(input);
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
            .unwrap_or_else(|| "Usage: /time set <day|noon|night|midnight|HHMM>".to_string()),
    }
}

fn unknown_command(input: &str) -> String {
    let suggestions = command_suggestions(input);
    if suggestions.is_empty() {
        format!("Unknown command: {}. Type /help", input)
    } else {
        format!(
            "Unknown command: {}. Did you mean {}?",
            input, suggestions[0].insert
        )
    }
}

fn command_suggestions(input: &str) -> Vec<CommandSuggestion> {
    if !input.starts_with('/') {
        return Vec::new();
    }

    COMMANDS
        .iter()
        .filter(|command| {
            command.insert.starts_with(input)
                || command.insert.split_whitespace().next() == Some(input)
        })
        .map(|command| CommandSuggestion {
            insert: command.insert,
            help: command.help,
        })
        .collect()
}

fn refresh_suggestions(state: &mut ChatState) {
    state.suggestions = command_suggestions(state.input.trim());
    state.suggestion_index = state
        .suggestion_index
        .min(state.suggestions.len().saturating_sub(1));
}

fn select_suggestion(state: &mut ChatState, direction: isize) {
    if state.suggestions.is_empty() {
        return;
    }

    let len = state.suggestions.len() as isize;
    state.suggestion_index = (state.suggestion_index as isize + direction).rem_euclid(len) as usize;
}

fn accept_suggestion(state: &mut ChatState) {
    let Some(suggestion) = state.suggestions.get(state.suggestion_index) else {
        return;
    };

    state.input = suggestion.insert.to_string();
    if !state.input.ends_with(' ') {
        state.input.push(' ');
    }
    refresh_suggestions(state);
}

fn push_history(state: &mut ChatState, message: String) {
    for line in message.lines() {
        state.history.push(line.to_string());
    }
    let keep_from = state.history.len().saturating_sub(8);
    state.history.drain(0..keep_from);
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
        KeyCode::Minus => '-',
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
        *panel = if state.open || state.message_timer > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let Ok(mut text) = texts.single_mut() else {
        return;
    };

    text.0 = if state.open {
        chat_open_text(&state)
    } else {
        state.history.join("\n")
    };
}

fn chat_open_text(state: &ChatState) -> String {
    let mut lines = Vec::new();
    if !state.history.is_empty() {
        lines.extend(state.history.iter().rev().take(4).rev().cloned());
    }
    lines.push(format!("> {}", state.input));

    if state.input.starts_with('/') {
        if state.suggestions.is_empty() {
            lines.push("No matching commands".to_string());
        } else {
            for (index, suggestion) in state.suggestions.iter().take(6).enumerate() {
                let prefix = if index == state.suggestion_index {
                    "> "
                } else {
                    "  "
                };
                lines.push(format!(
                    "{}{} - {}",
                    prefix, suggestion.insert, suggestion.help
                ));
            }
        }
    }

    lines.join("\n")
}
