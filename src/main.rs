use bevy::{
    prelude::*,
    window::{Cursor, CursorGrabMode},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_prototype_debug_lines::{DebugLinesPlugin, DebugShapes};
use grid_tree::{Level, NodeEntry, NodeKey, NodePtr, OctreeI32, VisitCommand};
use smooth_bevy_cameras::{
    controllers::fps::{FpsCameraBundle, FpsCameraController, FpsCameraPlugin},
    LookTransformPlugin,
};

const OCTREE_HEIGHT: Level = 10;
const DETAIL: i32 = 1;

const ROOT_COLOR: Color = Color::RED;
const NODE_COLOR: Color = Color::WHITE;
const LEAF_COLOR: Color = Color::GREEN;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                cursor: {
                    let mut cursor = Cursor::default();
                    cursor.visible = false;
                    cursor.grab_mode = CursorGrabMode::Locked;
                    cursor
                },
                ..default()
            }),
            ..default()
        }))
        .add_plugin(WorldInspectorPlugin::new())
        .add_plugin(DebugLinesPlugin::default())
        .add_plugin(LookTransformPlugin)
        .add_plugin(FpsCameraPlugin::default())
        .add_startup_system(init)
        .add_system(toggle_cursor_and_camera)
        .add_systems((move_target, update_octree, render).chain())
        .run()
}

trait CanSubdivide {
    fn can_subdivide(&self, node: Self, detail: i32) -> bool;
}

impl CanSubdivide for NodeKey<IVec3> {
    /// Adapted from https://github.com/Dimev/lodtree
    fn can_subdivide(&self, node_key: Self, detail: i32) -> bool {
        if node_key.level < self.level {
            return false;
        }

        let level_difference = node_key.level - self.level;
        let [s_x, s_y, s_z] = self.coordinates.to_array();
        let [n_x, n_y, n_z] = node_key.coordinates.to_array();

        // minimum corner of the bounding box
        let min = (
            (n_x << (level_difference + 1))
                .saturating_sub(((detail + 1) << level_difference) - (1 << level_difference)),
            (n_y << (level_difference + 1))
                .saturating_sub(((detail + 1) << level_difference) - (1 << level_difference)),
            (n_z << (level_difference + 1))
                .saturating_sub(((detail + 1) << level_difference) - (1 << level_difference)),
        );

        // max as well
        let max = (
            (n_x << (level_difference + 1))
                .saturating_add(((detail + 1) << level_difference) + (1 << level_difference)),
            (n_y << (level_difference + 1))
                .saturating_add(((detail + 1) << level_difference) + (1 << level_difference)),
            (n_z << (level_difference + 1))
                .saturating_add(((detail + 1) << level_difference) + (1 << level_difference)),
        );

        // local position of the target
        let local = (s_x << 1, s_y << 1, s_z << 1);

        // check if the target is inside of the bounding box
        local.0 >= min.0
            && local.0 < max.0
            && local.1 >= min.1
            && local.1 < max.1
            && local.2 >= min.2
            && local.2 < max.2
    }
}

fn init(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("Starting up!");

    let tree = OctreeI32::new(OCTREE_HEIGHT);
    let root_length = 2i32.pow(tree.root_level() as u32);
    commands.insert_resource(Octree(tree));

    info!(?root_length);

    // tree.fill_tree_from_root(
    //     NodeKey::new(tree.root_level(), IVec3::ZERO),
    //     0,
    //     |key, entry| {
    //         match entry {
    //             NodeEntry::Occupied(_) => {}
    //             NodeEntry::Vacant(v) => {
    //                 v.insert(());
    //             }
    //         }

    //         if target_key.can_subdivide(key, DETAIL) {
    //             VisitCommand::Continue
    //         } else {
    //             VisitCommand::SkipDescendants
    //         }
    //     },
    // );

    commands
        .spawn(Camera3dBundle::default())
        .insert(FpsCameraBundle::new(
            FpsCameraController {
                translate_sensitivity: root_length as f32 / 2.0, // we should take 2s to travel the root node
                ..default()
            },
            Vec3::splat(10.0),
            Vec3::ZERO,
            Vec3::Y,
        ));

    let sphere = meshes.add(
        Mesh::try_from(shape::Icosphere {
            radius: 0.5,
            subdivisions: 5,
        })
        .unwrap(),
    );
    commands.spawn(PbrBundle {
        mesh: sphere.clone(),
        material: materials.add(Color::RED.into()),
        transform: Transform::IDENTITY,
        ..default()
    });

    commands.spawn((
        Target,
        PbrBundle {
            mesh: sphere,
            material: materials.add(StandardMaterial {
                base_color: Color::GREEN,
                unlit: true,
                ..default()
            }),
            transform: Transform::from_translation(Vec3::new(
                (root_length / 2) as f32,
                (root_length / 2) as f32,
                0.0,
            )),
            ..default()
        },
    ));
}

#[derive(Resource, Debug, Deref, DerefMut)]
struct Octree(OctreeI32<()>);

#[derive(Component, Reflect)]
struct Target;

fn move_target(mut target_query: Query<&mut Transform, With<Target>>) {
    for mut transform in &mut target_query {
        transform.translate_around(
            Vec3::splat(2i32.pow((OCTREE_HEIGHT as u32 - 1) - 1) as f32),
            Quat::from_euler(EulerRot::XYZ, 0.005, 0.005, 0.005),
        )
    }
}

fn update_octree(mut tree: ResMut<Octree>, target_query: Query<&GlobalTransform, With<Target>>) {
    let target_pos = target_query.single();
    let target_key = NodeKey::new(0, target_pos.translation().as_ivec3());

    // Overwrite the current octree
    tree.0 = OctreeI32::new(OCTREE_HEIGHT);

    let root_key = NodeKey::new(tree.root_level(), IVec3::ZERO);
    tree.fill_tree_from_root(root_key, 0, |key, entry| {
        match entry {
            NodeEntry::Occupied(_) => {}
            NodeEntry::Vacant(v) => {
                v.insert(());
            }
        }

        if target_key.can_subdivide(key, DETAIL) {
            VisitCommand::Continue
        } else {
            VisitCommand::SkipDescendants
        }
    });
}

fn render(mut shapes: ResMut<DebugShapes>, tree: Res<Octree>) {
    tree.iter_roots()
        .map(|(root_key, root_node)| (root_key, NodePtr::new(root_key.level, root_node.self_ptr)))
        .for_each(|(root_key, root_ptr)| {
            tree.visit_tree_depth_first(
                root_ptr,
                root_key.coordinates,
                0,
                |child_ptr, child_coords| {
                    let scale_factor = 2i32.pow(child_ptr.level() as u32);
                    let child_min = child_coords * scale_factor;
                    let child_max = child_min + IVec3::splat(scale_factor);

                    let color = if child_ptr.level() == root_key.level {
                        ROOT_COLOR
                    } else if child_ptr.level() == 0 {
                        LEAF_COLOR
                    } else {
                        NODE_COLOR
                    };

                    shapes
                        .cuboid()
                        .min_max(child_min.as_vec3(), child_max.as_vec3())
                        .color(color);

                    VisitCommand::Continue
                },
            );
        });
}

fn toggle_cursor_and_camera(
    keys: Res<Input<KeyCode>>,
    mut windows: Query<&mut Window>,
    mut cameras: Query<&mut FpsCameraController>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        let mut window = windows.single_mut();
        window.cursor.visible = !window.cursor.visible;
        window.cursor.grab_mode = match window.cursor.grab_mode {
            CursorGrabMode::None => CursorGrabMode::Locked,
            CursorGrabMode::Confined | CursorGrabMode::Locked => CursorGrabMode::None,
        };

        let mut camera = cameras.single_mut();
        camera.enabled = !camera.enabled;
    }
}
