use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, Mesh},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use crate::game::camera::PlayerCamera;

pub struct SkyPlugin;

#[derive(Resource)]
pub struct LightingState {
    pub time_of_day: f32,
    pub clock_minutes: u32,
    pub day_factor: f32,
    pub sky_light: u8,
    pub block_light: u8,
    pub sun_angle: f32,
    pub label: &'static str,
}

#[derive(Component)]
struct SunDisc;

#[derive(Component)]
struct MoonDisc;

#[derive(Component)]
struct SkyDome;

#[derive(Component)]
struct Star {
    direction: Vec3,
    scale: f32,
}

#[derive(Component)]
struct CloudLayer {
    speed: f32,
    height: f32,
    offset: Vec2,
}

const DAY_LENGTH_SECONDS: f32 = 1200.0;

impl LightingState {
    pub fn set_clock(&mut self, hours: u32, minutes: u32) {
        let total = (hours.min(23) * 60 + minutes.min(59)) % 1440;
        self.time_of_day = total as f32 / 1440.0;
        self.clock_minutes = total;
    }
}

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LightingState {
            time_of_day: 0.5,
            clock_minutes: 12 * 60,
            day_factor: 1.0,
            sky_light: 15,
            block_light: 0,
            sun_angle: 0.0,
            label: "day",
        })
        .add_systems(Startup, spawn_sky)
        .add_systems(Update, (update_day_cycle, update_clouds));
    }
}

fn spawn_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.34),
        emissive: LinearRgba::rgb(4.0, 3.1, 1.0),
        unlit: true,
        double_sided: true,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(sky_dome_mesh(
            Color::srgb(0.5, 0.74, 1.0),
            Color::srgb(0.78, 0.9, 1.0),
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        SkyDome,
        NotShadowCaster,
    ));

    commands.spawn((
        Mesh3d(meshes.add(disc_mesh(15.0, 64))),
        MeshMaterial3d(sun_material),
        Transform::from_xyz(0.0, 70.0, -130.0),
        SunDisc,
        NotShadowCaster,
    ));

    let moon_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.84, 1.0),
        emissive: LinearRgba::rgb(0.7, 0.82, 1.4),
        unlit: true,
        double_sided: true,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(disc_mesh(6.0, 40))),
        MeshMaterial3d(moon_material),
        Transform::from_xyz(0.0, 90.0, 120.0),
        MoonDisc,
        NotShadowCaster,
    ));

    let star_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.9, 1.0),
        emissive: LinearRgba::rgb(1.8, 2.0, 2.8),
        unlit: true,
        double_sided: true,
        ..default()
    });
    let star_mesh = meshes.add(disc_mesh(0.85, 10));

    for index in 0..120 {
        let direction = star_direction(index);
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_material.clone()),
            Transform::from_translation(direction * 160.0),
            Star {
                direction,
                scale: 0.85 + (index % 5) as f32 * 0.14,
            },
            NotShadowCaster,
        ));
    }

    let cloud_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.68),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        ..default()
    });

    for index in 0..7 {
        let size = 50.0 + index as f32 * 7.0;
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(size, size * 0.34))),
            MeshMaterial3d(cloud_material.clone()),
            Transform::from_xyz(index as f32 * 30.0 - 90.0, 48.0 + index as f32 * 1.2, -34.0),
            CloudLayer {
                speed: 0.42 + index as f32 * 0.07,
                height: 48.0 + index as f32 * 1.2,
                offset: Vec2::new(index as f32 * 31.0, index as f32 * -17.0),
            },
            NotShadowCaster,
        ));
    }
}

fn update_day_cycle(
    time: Res<Time>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut lighting: ResMut<LightingState>,
    mut last_sky: Local<Option<([f32; 4], [f32; 4])>>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut domes: Query<(&Mesh3d, &mut Transform), (With<SkyDome>, Without<PlayerCamera>)>,
    mut suns: Query<
        &mut Transform,
        (
            With<SunDisc>,
            Without<SkyDome>,
            Without<MoonDisc>,
            Without<Star>,
            Without<PlayerCamera>,
        ),
    >,
    mut moons: Query<
        &mut Transform,
        (
            With<MoonDisc>,
            Without<SkyDome>,
            Without<SunDisc>,
            Without<Star>,
            Without<PlayerCamera>,
        ),
    >,
    mut stars: Query<
        (&Star, &mut Transform),
        (
            Without<SkyDome>,
            Without<SunDisc>,
            Without<MoonDisc>,
            Without<PlayerCamera>,
        ),
    >,
    mut lights: Query<
        (&mut DirectionalLight, &mut Transform),
        (
            Without<SunDisc>,
            Without<SkyDome>,
            Without<MoonDisc>,
            Without<Star>,
            Without<PlayerCamera>,
        ),
    >,
) {
    lighting.time_of_day = (lighting.time_of_day + time.delta_secs() / DAY_LENGTH_SECONDS) % 1.0;
    lighting.clock_minutes = ((lighting.time_of_day * 1440.0).round() as u32) % 1440;

    let angle = (lighting.time_of_day - 0.25) * std::f32::consts::TAU;
    let sun_height = angle.sin();
    let raw_day = ((sun_height + 0.28) / 0.9).clamp(0.0, 1.0);
    let day_factor = smooth_step(raw_day);
    let dusk = (1.0 - (day_factor - 0.45).abs() * 3.0).clamp(0.0, 1.0);
    let (sky_top, sky_horizon) = sky_colors(day_factor, dusk);
    let ambient_color = ambient_color(day_factor, dusk);
    let sun_direction = Vec3::new(angle.cos() * 0.55, sun_height, -1.0).normalize_or_zero();
    let moon_direction = -sun_direction;
    let night_factor = 1.0 - day_factor;
    let active_direction = if day_factor > 0.08 {
        sun_direction
    } else {
        moon_direction
    };

    clear.0 = sky_horizon;
    ambient.color = ambient_color;
    ambient.brightness = 210.0 + day_factor * 360.0;
    lighting.day_factor = day_factor;
    lighting.sky_light = (4.0 + day_factor * 11.0).round() as u8;
    lighting.block_light = 0;
    lighting.sun_angle = angle.to_degrees().rem_euclid(360.0);
    lighting.label = time_label(lighting.time_of_day, day_factor);

    let Ok(camera) = cameras.single() else {
        return;
    };

    for (mesh, mut transform) in &mut domes {
        transform.translation = camera.translation;
        let top = sky_top.to_linear().to_f32_array();
        let horizon = sky_horizon.to_linear().to_f32_array();
        let needs_update = last_sky
            .as_ref()
            .map(|last| color_delta(last.0, top) > 0.006 || color_delta(last.1, horizon) > 0.006)
            .unwrap_or(true);
        if needs_update {
            if let Some(mesh) = meshes.get_mut(&mesh.0) {
                update_sky_dome_colors(mesh, sky_top, sky_horizon);
            }
            *last_sky = Some((top, horizon));
        }
    }

    for (mut light, mut transform) in &mut lights {
        light.illuminance = if day_factor > 0.08 {
            3_000.0 + day_factor * 28_000.0
        } else {
            2_400.0
        };
        light.color = if day_factor > 0.08 {
            Color::srgb(1.0, 0.9 + day_factor * 0.1, 0.72 + day_factor * 0.28)
        } else {
            Color::srgb(0.42, 0.5, 0.8)
        };
        light.shadows_enabled = day_factor > 0.08 || night_factor > 0.55;
        transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, -active_direction);
    }

    for mut transform in &mut suns {
        transform.translation = camera.translation + sun_direction * 150.0;
        transform.look_at(camera.translation, Vec3::Y);
        transform.scale = Vec3::splat(day_factor.max(0.01));
    }

    for mut transform in &mut moons {
        transform.translation = camera.translation + moon_direction * 145.0;
        transform.look_at(camera.translation, Vec3::Y);
        transform.scale = Vec3::splat(night_factor.max(0.01));
    }

    for (star, mut transform) in &mut stars {
        transform.translation = camera.translation + star.direction * 135.0;
        transform.look_at(camera.translation, Vec3::Y);
        transform.scale = Vec3::splat(star.scale * night_factor.clamp(0.0, 1.0));
    }
}

fn update_clouds(
    time: Res<Time>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut clouds: Query<(&CloudLayer, &mut Transform), Without<PlayerCamera>>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };

    let elapsed = time.elapsed_secs();

    for (cloud, mut transform) in &mut clouds {
        let drift = Vec2::new(elapsed * cloud.speed, elapsed * cloud.speed * 0.32);
        let wrapped = wrap_cloud(cloud.offset + drift);
        transform.translation.x = camera.translation.x + wrapped.x;
        transform.translation.y = cloud.height;
        transform.translation.z = camera.translation.z + wrapped.y - 42.0;
        transform.rotation = Quat::IDENTITY;
    }
}

fn wrap_cloud(value: Vec2) -> Vec2 {
    Vec2::new(
        value.x.rem_euclid(150.0) - 75.0,
        value.y.rem_euclid(90.0) - 45.0,
    )
}

fn sky_colors(day_factor: f32, dusk: f32) -> (Color, Color) {
    (
        Color::srgb(
            0.025 + day_factor * 0.36 + dusk * 0.13,
            0.04 + day_factor * 0.5 + dusk * 0.07,
            0.09 + day_factor * 0.74,
        ),
        Color::srgb(
            0.08 + day_factor * 0.52 + dusk * 0.28,
            0.1 + day_factor * 0.64 + dusk * 0.15,
            0.16 + day_factor * 0.8 - dusk * 0.08,
        ),
    )
}

fn ambient_color(day_factor: f32, dusk: f32) -> Color {
    Color::srgb(
        0.28 + day_factor * 0.36 + dusk * 0.12,
        0.3 + day_factor * 0.42 + dusk * 0.06,
        0.42 + day_factor * 0.42,
    )
}

fn time_label(time_of_day: f32, day_factor: f32) -> &'static str {
    let minutes = (time_of_day * 1440.0) as u32;
    match minutes {
        300..=659 => "morning",
        660..=1019 => "day",
        1020..=1259 => "evening",
        _ if day_factor < 0.18 => "night",
        _ => "night",
    }
}

fn smooth_step(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn sky_dome_mesh(top: Color, horizon: Color) -> Mesh {
    let radius = 205.0f32;
    let rings = 8usize;
    let segments = 36usize;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let theta = v * std::f32::consts::FRAC_PI_2;
        let y = theta.sin() * radius - 18.0;
        let r = theta.cos() * radius;
        let color = mix_color(horizon, top, v);

        for segment in 0..=segments {
            let a = segment as f32 / segments as f32 * std::f32::consts::TAU;
            positions.push([a.cos() * r, y, a.sin() * r]);
            normals.push([0.0f32, -1.0, 0.0]);
            colors.push(color.to_linear().to_f32_array());
        }
    }

    for ring in 0..rings {
        for segment in 0..segments {
            let row = segments + 1;
            let a = ring * row + segment;
            let b = a + 1;
            let c = a + row;
            let d = c + 1;
            indices
                .extend_from_slice(&[a as u32, c as u32, b as u32, b as u32, c as u32, d as u32]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn update_sky_dome_colors(mesh: &mut Mesh, top: Color, horizon: Color) {
    let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return;
    };
    let Some(values) = positions.as_float3() else {
        return;
    };
    let colors: Vec<[f32; 4]> = values
        .iter()
        .map(|p| {
            let v = ((p[1] + 18.0) / 205.0).clamp(0.0, 1.0);
            mix_color(horizon, top, v).to_linear().to_f32_array()
        })
        .collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_linear();
    let b = b.to_linear();
    Color::linear_rgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        1.0,
    )
}

fn color_delta(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

fn disc_mesh(radius: f32, segments: usize) -> Mesh {
    let mut positions = Vec::with_capacity(segments + 1);
    let mut normals = Vec::with_capacity(segments + 1);
    let mut uvs = Vec::with_capacity(segments + 1);
    let mut indices = Vec::with_capacity(segments * 3);

    positions.push([0.0f32, 0.0, 0.0]);
    normals.push([0.0f32, 0.0, 1.0]);
    uvs.push([0.5f32, 0.5]);

    for index in 0..segments {
        let angle = index as f32 / segments as f32 * std::f32::consts::TAU;
        let x = angle.cos() * radius;
        let y = angle.sin() * radius;
        positions.push([x, y, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([(x / radius + 1.0) * 0.5, (y / radius + 1.0) * 0.5]);
    }

    for index in 1..=segments {
        let next = if index == segments { 1 } else { index + 1 };
        indices.extend_from_slice(&[0, index as u32, next as u32]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn star_direction(index: usize) -> Vec3 {
    let a = index as f32 * 2.399_963;
    let y = 0.18 + ((index * 37 % 82) as f32 / 82.0) * 0.78;
    let r = (1.0 - y * y).sqrt();
    Vec3::new(a.cos() * r, y, a.sin() * r).normalize()
}
