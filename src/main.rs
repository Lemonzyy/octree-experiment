use std::time::Instant;

use bevy::prelude::*;
use bevy_prototype_debug_lines::{DebugLinesPlugin, DebugShapes};
use grid_tree::{Level, NodeEntry, NodeKey, NodePtr, OctreeI32, VisitCommand};
use rand::Rng;
use smooth_bevy_cameras::{
    controllers::fps::{FpsCameraBundle, FpsCameraController, FpsCameraPlugin},
    LookTransformPlugin,
};

const OCTREE_HEIGHT: Level = 10;
const PATH_NUM: u32 = 5;

const ROOT_COLOR: Color = Color::RED;
const NODE_COLOR: Color = Color::WHITE;
const LEAF_COLOR: Color = Color::GREEN;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(DebugLinesPlugin::default())
        .add_plugin(LookTransformPlugin)
        .add_plugin(FpsCameraPlugin::default())
        .add_startup_system(init)
        .add_system(render)
        .run()
}

fn init(mut commands: Commands) {
    info!("Starting up!");

    let mut tree = OctreeI32::new(OCTREE_HEIGHT);
    let half_root_length = 2i32.pow(tree.root_level() as u32);

    let mut rand = rand::thread_rng();
    let mut gen = || rand.gen_range(-half_root_length..half_root_length);

    let start = Instant::now();
    for _ in 0..PATH_NUM {
        let target_key = NodeKey::new(0, IVec3::new(gen(), gen(), gen()));

        tree.fill_path_to_node_from_root(target_key, |_key, entry| {
            match entry {
                NodeEntry::Occupied(_) => {}
                NodeEntry::Vacant(v) => {
                    v.insert(());
                }
            }
            VisitCommand::Continue
        });
    }
    info!(
        "took {:.2?} to fill {}x path(s) to a random location from the root",
        start.elapsed(),
        PATH_NUM
    );

    commands.insert_resource(Octree(tree));

    commands
        .spawn(Camera3dBundle::default())
        .insert(FpsCameraBundle::new(
            FpsCameraController {
                translate_sensitivity: half_root_length as f32, // we should take 1s to travel half the root length
                ..default()
            },
            Vec3::splat(10.0),
            Vec3::ZERO,
            Vec3::Y,
        ));
}

#[derive(Resource, Debug, Deref)]
struct Octree(OctreeI32<()>);

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
