use std::path::PathBuf;

use bevy::{
    app::AppExit,
    diagnostic::FrameTimeDiagnosticsPlugin,
    ecs::system::Command,
    prelude::*,
    reflect::{erased_serde::private::serde::de::DeserializeSeed, TypeRegistry},
    render::pipeline::IndexFormat,
    scene::serde::SceneDeserializer,
    wgpu::{WgpuFeature, WgpuFeatures, WgpuOptions},
};
use bevy_editor_pls::{extensions::EditorExtensionSpawn, EditorPlugin, EditorSettings};
use derive_more::{Deref, DerefMut};

fn editor_settings() -> EditorSettings {
    let mut settings = EditorSettings::default();
    settings.auto_pickable = true;
    settings.auto_flycam = true;

    settings.add_event("Quit", || AppExit);

    settings
}

#[derive(Default, Deref, DerefMut)]
struct OpenScene(Option<PathBuf>);
#[derive(Default, Deref, DerefMut)]
struct ChangeSinceLastSave(bool);

fn main() {
    App::build()
        .insert_resource(WgpuOptions {
            features: WgpuFeatures {
                features: vec![WgpuFeature::NonFillPolygonMode],
            },
            ..Default::default()
        })
        .register_type::<Option<IndexFormat>>()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(editor_settings())
        .add_plugins(DefaultPlugins)
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_plugin(EditorPlugin)
        .add_plugin(EditorExtensionSpawn)
        .init_resource::<OpenScene>()
        .init_resource::<ChangeSinceLastSave>()
        .add_startup_system(bevy_editor_pls::setup_default_keybindings.system())
        // systems
        .add_startup_system(setup.system())
        .add_system(title_adjust.system())
        .add_system(save.system())
        .run();
}

fn title_adjust(open_path: Res<OpenScene>, mut windows: ResMut<Windows>, csls: Res<ChangeSinceLastSave>) {
    if !open_path.is_changed() && !csls.is_changed() {
        return;
    }
    let window = windows.get_primary_mut().unwrap();
    window.set_title(format!(
        "Bevy editor - {}{}",
        if **csls { "*" } else { "" },
        open_path.as_ref().map(|v| v.to_string_lossy()).unwrap_or("Unsaved".into())
    ))
}

fn save(input: Res<Input<KeyCode>>, mut commands: Commands, mut open_scene: ResMut<OpenScene>) {
    if input.pressed(KeyCode::LControl) {
        if input.just_pressed(KeyCode::S) {
            if input.pressed(KeyCode::LShift) {
                if open_scene.is_none() {
                    let path = native_dialog::FileDialog::new().show_save_single_file().unwrap();
                    if let Some(mut p) = path {
                        p.set_extension("scn.ron");
                        **open_scene = Some(p);
                    }
                }
            }
            commands.add(SaveCommand);
        } else if input.just_pressed(KeyCode::O) {
            commands.add(OpenCommand);
        }
    }
}

struct SaveCommand;
struct OpenCommand;

impl Command for SaveCommand {
    fn write(self: Box<Self>, world: &mut World) {
        let type_registry = world.get_resource::<TypeRegistry>().unwrap();
        let scene = DynamicScene::from_world(&world, &type_registry);

        let serialized = scene.serialize_ron(&type_registry).unwrap();
        let mut open_scene = world.get_resource_mut::<OpenScene>().unwrap();
        if open_scene.is_none() {
            let path = native_dialog::FileDialog::new().show_save_single_file().unwrap();
            if let Some(mut p) = path {
                p.set_extension("scn.ron");
                **open_scene = Some(p);
            } else {
                return;
            }
        }
        std::fs::write(open_scene.as_ref().unwrap(), serialized).unwrap();
        **world.get_resource_mut::<ChangeSinceLastSave>().unwrap() = false;
    }
}

impl Command for OpenCommand {
    fn write(self: Box<Self>, world: &mut World) {
        let path = native_dialog::FileDialog::new().show_open_single_file().unwrap();
        let path = if let Some(p) = path {
            p
        } else {
            return;
        };
        let file = std::fs::read(&path).unwrap();
        let mut deserializer = ron::de::Deserializer::from_bytes(&file).unwrap();
        let registry = world.get_resource::<TypeRegistry>().unwrap().read();
        let scene_deserializer = SceneDeserializer {
            type_registry: &*registry,
        };
        let scene = scene_deserializer.deserialize(&mut deserializer).unwrap();
        drop(registry);
        let to_despawn = world.query::<Entity>().iter(world).collect::<Vec<_>>();
        for e in to_despawn {
            world.entity_mut(e).despawn();
        }
        scene.write_to_world(world, &mut Default::default()).unwrap();
        **world.get_resource_mut::<OpenScene>().unwrap() = Some(path);
    }
}

pub fn setup(mut commands: Commands, mut _meshes: ResMut<Assets<Mesh>>, mut _materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}
