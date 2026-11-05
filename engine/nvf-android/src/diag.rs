//! The diagnostic screen: what Milestone 1 was, kept as the fallback. It starts instead of
//! the game when the game cannot start (no `Data` folder, an incomplete copy, schemas that
//! cannot be written), because on a phone a game that quits at once says nothing.
//!
//! It draws a spinning cube (proof that Vulkan and touch work), the reason the game did not
//! start, and the data probe's report (`probe.rs`: opens every archive and inflates a mesh).

use crate::paths;
use crate::probe::{self, ProbeReport};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::tasks::futures_lite::future;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::window::{MonitorSelection, WindowMode};
use bevy::winit::WinitSettings;

/// Why the game did not start (shown on screen).
#[derive(Resource)]
struct Reason(String);

#[derive(Resource)]
struct ProbeTask(Task<ProbeReport>);

#[derive(Resource, Default)]
struct ProbeText(String);

#[derive(Component)]
struct Spinner;

#[derive(Component)]
struct Overlay;

/// Runs the diagnostic app until the user closes it.
pub fn run(reason: String) {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: RenderCreation::from(WgpuSettings { backends: Some(Backends::VULKAN), ..default() }),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "NVForever".into(),
                        resizable: false,
                        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .insert_resource(WinitSettings::mobile())
        .insert_resource(Reason(reason))
        .add_systems(Startup, (setup_scene, start_probe))
        .add_systems(Update, (poll_probe, spin_with_touch, update_overlay))
        .run();
}

fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 0.8, 4.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((DirectionalLight { illuminance: 12_000.0, ..default() }, Transform::from_xyz(3.0, 5.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.4, 1.4, 1.4))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.65, 0.25), perceptual_roughness: 0.6, ..default() })),
        Transform::default(),
        Spinner,
    ));
    commands.insert_resource(ProbeText("Probing game data...".into()));
    commands.spawn((
        Text::new("NVForever"),
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, top: Val::Px(48.0), left: Val::Px(16.0), right: Val::Px(16.0), ..default() },
        Overlay,
    ));
}

fn start_probe(mut commands: Commands) {
    let dir = paths::data_dir();
    let task = AsyncComputeTaskPool::get().spawn(async move { probe::probe(&dir) });
    commands.insert_resource(ProbeTask(task));
}

fn poll_probe(mut commands: Commands, task: Option<ResMut<ProbeTask>>, mut text: ResMut<ProbeText>) {
    let Some(mut task) = task else { return };
    if let Some(report) = future::block_on(future::poll_once(&mut task.0)) {
        info!("NVForever: data probe finished:\n{report}");
        text.0 = report.to_string();
        commands.remove_resource::<ProbeTask>();
    }
}

fn spin_with_touch(time: Res<Time>, touches: Res<Touches>, mut cube: Query<&mut Transform, With<Spinner>>) {
    let Ok(mut t) = cube.single_mut() else { return };
    let mut dragged = false;
    for touch in touches.iter() {
        let d = touch.delta();
        if d != Vec2::ZERO {
            dragged = true;
            t.rotate_y(d.x * 0.01);
            t.rotate_x(d.y * 0.01);
        }
    }
    if !dragged {
        t.rotate_y(0.6 * time.delta_secs());
    }
}

fn update_overlay(diagnostics: Res<DiagnosticsStore>, reason: Res<Reason>, probe: Res<ProbeText>, mut overlay: Query<&mut Text, With<Overlay>>) {
    let Ok(mut text) = overlay.single_mut() else { return };
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    **text = format!("NVForever (Android) - the game did not start   {fps:.0} fps\n\n{}\n\n{}", reason.0, probe.0);
}
