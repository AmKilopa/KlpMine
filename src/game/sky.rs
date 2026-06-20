use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemParam,
    light::NotShadowCaster,
    mesh::{Indices, Mesh},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use crate::game::camera::PlayerCamera;
use crate::game::settings::GameSettings;

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
struct SunGlow;

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
    wrap: Vec2,
}

const DAY_LENGTH_SECONDS: f32 = 1200.0;

type SkyCameraQuery<'w, 's> = Query<'w, 's, &'static Transform, With<PlayerCamera>>;
type SkyDomeQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Mesh3d, &'static mut Transform),
    (With<SkyDome>, Without<PlayerCamera>),
>;
type SunDiscQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<SunDisc>,
        Without<SunGlow>,
        Without<SkyDome>,
        Without<MoonDisc>,
        Without<Star>,
        Without<PlayerCamera>,
    ),
>;
type SunGlowQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<SunGlow>,
        Without<SunDisc>,
        Without<SkyDome>,
        Without<MoonDisc>,
        Without<Star>,
        Without<PlayerCamera>,
    ),
>;
type MoonQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<MoonDisc>,
        Without<SunGlow>,
        Without<SkyDome>,
        Without<SunDisc>,
        Without<Star>,
        Without<PlayerCamera>,
    ),
>;
type StarQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Star, &'static mut Transform),
    (
        Without<SkyDome>,
        Without<SunDisc>,
        Without<SunGlow>,
        Without<MoonDisc>,
        Without<PlayerCamera>,
    ),
>;
type SkyLightQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut DirectionalLight, &'static mut Transform),
    (
        Without<SunDisc>,
        Without<SunGlow>,
        Without<SkyDome>,
        Without<MoonDisc>,
        Without<Star>,
        Without<PlayerCamera>,
    ),
>;

#[derive(SystemParam)]
struct SkyQueries<'w, 's> {
    cameras: SkyCameraQuery<'w, 's>,
    domes: SkyDomeQuery<'w, 's>,
    suns: SunDiscQuery<'w, 's>,
    sun_glows: SunGlowQuery<'w, 's>,
    moons: MoonQuery<'w, 's>,
    stars: StarQuery<'w, 's>,
    lights: SkyLightQuery<'w, 's>,
}

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
        base_color: Color::srgb(1.0, 0.85, 0.4),
        emissive: LinearRgba::rgb(8.0, 5.5, 1.5),
        unlit: true,
        double_sided: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let sun_glow_outer = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.6, 0.2, 0.12),
        emissive: LinearRgba::rgb(3.0, 1.5, 0.3),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        ..default()
    });

    let sun_glow_inner = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.8, 0.4, 0.22),
        emissive: LinearRgba::rgb(5.0, 3.0, 0.8),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(sky_dome_mesh(
            Color::srgb(0.55, 0.82, 1.0),
            Color::srgb(0.82, 0.92, 1.0),
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
        Mesh3d(meshes.add(disc_mesh(60.0, 72))),
        MeshMaterial3d(sun_glow_outer),
        Transform::from_xyz(0.0, 70.0, -130.0),
        SunGlow,
        NotShadowCaster,
    ));

    commands.spawn((
        Mesh3d(meshes.add(disc_mesh(28.0, 64))),
        MeshMaterial3d(sun_glow_inner),
        Transform::from_xyz(0.0, 70.0, -130.0),
        SunGlow,
        NotShadowCaster,
    ));

    commands.spawn((
        Mesh3d(meshes.add(disc_mesh(18.0, 64))),
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
        base_color: Color::srgb(0.9, 0.95, 1.0),
        emissive: LinearRgba::rgb(4.0, 4.5, 6.0),
        unlit: true,
        double_sided: true,
        ..default()
    });
    let star_mesh = meshes.add(disc_mesh(0.45, 10));

    for index in 0..350 {
        let direction = star_direction(index);
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_material.clone()),
            Transform::from_translation(direction * 160.0),
            Star {
                direction,
                scale: 0.65 + (index % 7) as f32 * 0.11,
            },
            NotShadowCaster,
        ));
    }

    let cloud_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.92, 0.95, 1.0, 0.85),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    for index in 0..32 {
        let px = (index % 8) as f32;
        let py = (index / 8) as f32;
        let width = 60.0 + px * 18.0;
        let depth = 30.0 + py * 12.0;
        let h = 54.0 + (index % 7) as f32 * 3.5;
        commands.spawn((
            Mesh3d(meshes.add(cloud_mesh(width, depth, index as u32))),
            MeshMaterial3d(cloud_material.clone()),
            Transform::from_xyz(
                (px * 40.0 - 160.0) + (index % 3) as f32 * 10.0,
                h,
                (py * 38.0 - 76.0) + (index % 5) as f32 * -8.0,
            ),
            CloudLayer {
                speed: 0.15 + (index % 12) as f32 * 0.03,
                height: h,
                offset: Vec2::new(index as f32 * 28.0, index as f32 * -22.0),
                wrap: Vec2::new(360.0, 320.0),
            },
            NotShadowCaster,
        ));
    }

    let cirrus_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.35),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    for index in 0..16 {
        let px = (index % 4) as f32;
        let py = (index / 4) as f32;
        let width = 100.0 + px * 30.0;
        let depth = 40.0 + py * 10.0;
        let h = 105.0 + (index % 5) as f32 * 6.0;
        commands.spawn((
            Mesh3d(meshes.add(cirrus_mesh(width, depth, index as u32))),
            MeshMaterial3d(cirrus_material.clone()),
            Transform::from_xyz(
                (px * 60.0 - 120.0) + (index % 3) as f32 * 15.0,
                h,
                (py * 80.0 - 80.0) + (index % 5) as f32 * -12.0,
            ),
            CloudLayer {
                speed: 0.4 + (index % 8) as f32 * 0.05,
                height: h,
                offset: Vec2::new(index as f32 * 50.0, index as f32 * -35.0),
                wrap: Vec2::new(480.0, 400.0),
            },
            NotShadowCaster,
        ));
    }

    info!("sky: sun glow=on stars=350 clouds=32 cirrus=16 shadows=on");
}

fn update_day_cycle(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut lighting: ResMut<LightingState>,
    mut last_sky: Local<Option<([f32; 4], [f32; 4])>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut sky: SkyQueries,
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
    ambient.brightness = 480.0 + day_factor * 820.0 + dusk * 160.0;
    lighting.day_factor = day_factor;
    lighting.sky_light = (4.0 + day_factor * 11.0).round() as u8;
    lighting.block_light = 0;
    lighting.sun_angle = angle.to_degrees().rem_euclid(360.0);
    lighting.label = time_label(lighting.time_of_day, day_factor);

    let Ok(camera) = sky.cameras.single() else {
        return;
    };

    for (mesh, mut transform) in &mut sky.domes {
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

    for (mut light, mut transform) in &mut sky.lights {
        light.illuminance = if day_factor > 0.08 {
            2_400.0 + day_factor * 6_800.0 + dusk * 2_000.0
        } else {
            560.0
        };
        light.color = if day_factor > 0.08 {
            Color::srgb(0.98, 0.88 + day_factor * 0.08 + dusk * 0.22, 0.74 + day_factor * 0.14 - dusk * 0.08)
        } else {
            Color::srgb(0.52, 0.6, 0.86)
        };
        light.shadows_enabled = settings.shadows;
        transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, -active_direction);
    }

    for mut transform in &mut sky.suns {
        transform.translation = camera.translation + sun_direction * 150.0;
        transform.look_at(camera.translation, Vec3::Y);
        transform.scale = Vec3::splat(day_factor.max(0.01));
    }

    for mut transform in &mut sky.sun_glows {
        transform.translation = camera.translation + sun_direction * 148.0;
        transform.look_at(camera.translation, Vec3::Y);
        let glow_boost = 1.0 + dusk * 0.6;
        transform.scale = Vec3::splat(day_factor.max(0.01) * glow_boost);
    }

    for mut transform in &mut sky.moons {
        transform.translation = camera.translation + moon_direction * 145.0;
        transform.look_at(camera.translation, Vec3::Y);
        transform.scale = Vec3::splat(night_factor.max(0.01));
    }

    for (star, mut transform) in &mut sky.stars {
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
        let drift = Vec2::new(elapsed * cloud.speed, elapsed * cloud.speed * 0.45);
        let wrapped = wrap_cloud(cloud.offset + drift, cloud.wrap);
        transform.translation.x = camera.translation.x + wrapped.x;
        transform.translation.y = cloud.height;
        transform.translation.z = camera.translation.z + wrapped.y;
        transform.rotation = Quat::IDENTITY;
    }
}

fn wrap_cloud(value: Vec2, wrap: Vec2) -> Vec2 {
    Vec2::new(
        value.x.rem_euclid(wrap.x) - wrap.x * 0.5,
        value.y.rem_euclid(wrap.y) - wrap.y * 0.5,
    )
}

fn sky_colors(day_factor: f32, dusk: f32) -> (Color, Color) {
    let sunset_r = 0.03 + day_factor * 0.38 + dusk * 0.35;
    let sunset_g = 0.04 + day_factor * 0.56 + dusk * 0.18;
    let sunset_b = 0.10 + day_factor * 0.78 - dusk * 0.05;
    (
        Color::srgb(
            sunset_r,
            sunset_g.clamp(0.04, 0.95),
            sunset_b.clamp(0.05, 0.88),
        ),
        Color::srgb(
            0.08 + day_factor * 0.54 + dusk * 0.42,
            0.10 + day_factor * 0.64 + dusk * 0.28,
            0.18 + day_factor * 0.75 - dusk * 0.08,
        ),
    )
}

fn ambient_color(day_factor: f32, dusk: f32) -> Color {
    Color::srgb(
        0.48 + day_factor * 0.28 + dusk * 0.18,
        0.5 + day_factor * 0.3 + dusk * 0.08,
        0.62 + day_factor * 0.28 - dusk * 0.04,
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
    let rings = 64usize;
    let segments = 64usize;
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

fn cloud_mesh(width: f32, depth: f32, seed: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for part in 0..30 {
        let u = cloud_unit(seed, part * 3 + 1);
        let v = cloud_unit(seed, part * 3 + 2);
        let biased_u = 0.5 + (u - 0.5) * 0.65;
        let biased_v = 0.5 + (v - 0.5) * 0.65;
        let cx = (biased_u - 0.5) * width * 0.75;
        let cz = (biased_v - 0.5) * depth * 0.85;
        let w2 = width * (0.08 + cloud_unit(seed, part * 3 + 3) * 0.28) * 0.5;
        let d2 = depth * (0.12 + cloud_unit(seed, part * 3 + 4) * 0.32) * 0.5;
        let h = 2.0 + cloud_unit(seed, part * 3 + 5) * 4.5;
        let yc = (cloud_unit(seed, part * 3 + 6) - 0.5) * h * 0.7;
        let yo = yc;
        let yt = yc + h;
        let minx = cx - w2;
        let maxx = cx + w2;
        let minz = cz - d2;
        let maxz = cz + d2;

        let box_faces = [
            ([minx, yo, minz], [maxx, yo, minz], [maxx, yo, maxz], [minx, yo, maxz], [0.0, -1.0, 0.0]),
            ([minx, yt, minz], [maxx, yt, minz], [maxx, yt, maxz], [minx, yt, maxz], [0.0, 1.0, 0.0]),
            ([minx, yo, minz], [maxx, yo, minz], [maxx, yt, minz], [minx, yt, minz], [0.0, 0.0, -1.0]),
            ([maxx, yo, maxz], [minx, yo, maxz], [minx, yt, maxz], [maxx, yt, maxz], [0.0, 0.0, 1.0]),
            ([minx, yo, maxz], [minx, yo, minz], [minx, yt, minz], [minx, yt, maxz], [-1.0, 0.0, 0.0]),
            ([maxx, yo, minz], [maxx, yo, maxz], [maxx, yt, maxz], [maxx, yt, minz], [1.0, 0.0, 0.0]),
        ];

        for (v0, v1, v2, v3, normal) in &box_faces {
            let face_base = positions.len() as u32;
            positions.extend_from_slice(&[*v0, *v1, *v2, *v3]);
            for _ in 0..4 {
                normals.push(*normal);
                uvs.push([0.0, 0.0]);
            }
            indices.extend_from_slice(&[
                face_base, face_base + 1, face_base + 2,
                face_base, face_base + 2, face_base + 3,
            ]);
        }
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

fn cirrus_mesh(width: f32, depth: f32, seed: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for part in 0..12 {
        let u = cloud_unit(seed, part * 2 + 1);
        let v = cloud_unit(seed, part * 2 + 2);
        let cx = (u - 0.5) * width * 0.9;
        let cz = (v - 0.5) * depth * 0.9;
        let w = width * (0.15 + cloud_unit(seed, part * 2 + 3) * 0.45);
        let d = depth * (0.03 + cloud_unit(seed, part * 2 + 4) * 0.05);
        let h = 0.3 + cloud_unit(seed, part * 2 + 5) * 0.5;
        let yc = (cloud_unit(seed, part * 2 + 7) - 0.5) * h * 0.6;
        let stretch = 0.3 + cloud_unit(seed, part * 2 + 6) * 0.7;
        let angle = cloud_unit(seed, part * 2 + 8) * std::f32::consts::TAU;
        let (sa, ca) = angle.sin_cos();
        let rx = w * 0.5;
        let rz = d * 0.5;

        let corners = [
            [cx + rx * ca - rz * sa * stretch, cz + rx * sa + rz * ca * stretch],
            [cx - rx * ca - rz * sa * stretch, cz - rx * sa + rz * ca * stretch],
            [cx - rx * ca + rz * sa * stretch, cz - rx * sa - rz * ca * stretch],
            [cx + rx * ca + rz * sa * stretch, cz + rx * sa - rz * ca * stretch],
        ];

        let yo = yc;
        let yt = yc + h;

        let box_faces = [
            ([corners[0][0], yo, corners[0][1]], [corners[1][0], yo, corners[1][1]], [corners[3][0], yo, corners[3][1]], [corners[2][0], yo, corners[2][1]], [0.0, -1.0, 0.0]),
            ([corners[0][0], yt, corners[0][1]], [corners[1][0], yt, corners[1][1]], [corners[3][0], yt, corners[3][1]], [corners[2][0], yt, corners[2][1]], [0.0, 1.0, 0.0]),
            ([corners[0][0], yo, corners[0][1]], [corners[1][0], yo, corners[1][1]], [corners[1][0], yt, corners[1][1]], [corners[0][0], yt, corners[0][1]], [0.0, 0.0, -1.0]),
            ([corners[2][0], yo, corners[2][1]], [corners[3][0], yo, corners[3][1]], [corners[3][0], yt, corners[3][1]], [corners[2][0], yt, corners[2][1]], [0.0, 0.0, 1.0]),
            ([corners[0][0], yo, corners[0][1]], [corners[2][0], yo, corners[2][1]], [corners[2][0], yt, corners[2][1]], [corners[0][0], yt, corners[0][1]], [-1.0, 0.0, 0.0]),
            ([corners[1][0], yo, corners[1][1]], [corners[3][0], yo, corners[3][1]], [corners[3][0], yt, corners[3][1]], [corners[1][0], yt, corners[1][1]], [1.0, 0.0, 0.0]),
        ];

        for (v0, v1, v2, v3, normal) in &box_faces {
            let base = positions.len() as u32;
            positions.extend_from_slice(&[*v0, *v1, *v2, *v3]);
            for _ in 0..4 {
                normals.push(*normal);
                uvs.push([0.0, 0.0]);
            }
            indices.extend_from_slice(&[
                base, base + 1, base + 2,
                base, base + 2, base + 3,
            ]);
        }
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

fn cloud_unit(seed: u32, salt: u32) -> f32 {
    let mut value = seed
        .wrapping_mul(1_664_525)
        .wrapping_add(salt.wrapping_mul(1_013_904_223));
    value ^= value >> 16;
    value = value.wrapping_mul(2_246_822_519);
    (value & 0xffff) as f32 / 65_535.0
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
    let y = 0.12 + ((index * 37 % 88) as f32 / 88.0) * 0.82;
    let r = (1.0 - y * y).sqrt();
    Vec3::new(a.cos() * r, y, a.sin() * r).normalize()
}
