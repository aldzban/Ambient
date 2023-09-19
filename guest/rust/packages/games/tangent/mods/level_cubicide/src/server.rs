use std::{collections::HashMap, f32::consts::TAU};

use ambient_api::{
    core::{
        app::components::main_scene,
        physics::components::{cube_collider, dynamic, mass, physics_controlled, plane_collider},
        primitives::components::{cube, quad},
        rendering::components::{
            cast_shadows, color, fog_color, fog_density, fog_height_falloff, light_diffuse, sky,
            sun,
        },
        transform::components::{rotation, scale, translation},
    },
    ecs::GeneralQuery,
    once_cell::sync::Lazy,
    prelude::*,
    rand,
};
use packages::tangent_schema::concepts::Spawnpoint;

use crate::packages::pickup_health::{components::is_health_pickup, concepts::HealthPickup};

mod shared;

const LEVEL_RADIUS: f32 = 125.;

#[main]
pub async fn main() {
    // Make sky
    Entity::new().with(sky(), ()).spawn();

    // Make sun
    let sky_color = vec3(0.11, 0.20, 0.27);
    Entity::new()
        .with(sun(), 0.0)
        .with(rotation(), Quat::from_rotation_y(10f32.to_radians()))
        .with(main_scene(), ())
        .with(light_diffuse(), sky_color * 2.)
        .with(fog_color(), sky_color)
        .with(fog_density(), 0.05)
        .with(fog_height_falloff(), 0.05)
        .spawn();

    // Make ground
    Entity::new()
        .with(quad(), ())
        .with(physics_controlled(), ())
        .with(plane_collider(), ())
        .with(dynamic(), false)
        .with(scale(), Vec3::ONE * 4000.)
        .with(color(), sky_color.extend(1.0))
        .spawn();

    // Spawn spawnpoints
    for (pos, radius, color) in shared::spawnpoints().iter().copied() {
        Spawnpoint {
            is_spawnpoint: (),
            radius,
            translation: pos,
            color: color.extend(1.0),
        }
        .spawn();
    }

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    make_cubes(&mut rng);
    handle_pickups(&mut rng);
}

fn make_cubes(rng: &mut dyn rand::RngCore) {
    const TARGET_CUBE_COUNT: usize = 1000;
    const CUBE_MIN_SIZE: Vec3 = vec3(0.5, 0.5, 0.5);
    const CUBE_MAX_SIZE: Vec3 = vec3(5., 6., 15.);
    const FADE_DISTANCE: f32 = 2.;

    // Spawn cubes until we hit the limit
    let mut grid = Grid::default();
    while grid.size() < TARGET_CUBE_COUNT {
        let position =
            shared::circle_point(rng.gen::<f32>() * TAU, rng.gen::<f32>() * LEVEL_RADIUS);

        let base_size = vec3(rng.gen(), rng.gen(), rng.gen());
        let size = base_size * (CUBE_MAX_SIZE - CUBE_MIN_SIZE) + CUBE_MIN_SIZE;
        let radius = size.xy().max_element();

        let level = shared::level(position);
        if level < radius {
            continue;
        }

        let probability = ((level - radius) / FADE_DISTANCE).clamp(0.0, 1.0);
        let sample = rng.gen::<f32>();
        if sample > probability {
            continue;
        }

        if grid.would_collide(position, radius) {
            continue;
        }

        make_cube(position, size, true, rng);
        grid.add(position, radius);
    }

    // Make surrounding walls
    for i in 0..360 {
        let angle = (i as f32).to_radians();
        let radius = LEVEL_RADIUS + rng.gen::<f32>() * 10.0;
        let position = shared::circle_point(angle, radius);

        let size = vec3(1.5, 1.5, 10.) + rng.gen::<Vec3>() * vec3(1., 1., 20.);
        make_cube(position, size, false, rng);
    }
}

fn handle_pickups(rng: &mut dyn rand::RngCore) {
    make_pickups(rng);

    // Make pickups respawn
    fixed_rate_tick(Duration::from_secs(5), |_| {
        make_pickups(&mut thread_rng());
    });
}

fn make_pickups(rng: &mut dyn rand::RngCore) {
    let pickup_count = shared::spawnpoints().len() * 2;

    static QUERY: Lazy<GeneralQuery<Component<Vec3>>> =
        Lazy::new(|| query(translation()).requires(is_health_pickup()).build());

    // Consider a more efficient scheme for more pickups
    loop {
        let existing_pickups = QUERY.evaluate();
        if existing_pickups.len() >= pickup_count {
            break;
        }

        let position =
            shared::circle_point(rng.gen::<f32>() * TAU, rng.gen::<f32>() * LEVEL_RADIUS);

        let level = shared::level(position.xy());
        if level > 0.0 {
            continue;
        }

        let position = position.extend(1.5);

        if existing_pickups
            .into_iter()
            .map(|(_, pos)| position.distance_squared(pos))
            .any(|length| length < 25f32.powi(2))
        {
            continue;
        }

        HealthPickup {
            is_health_pickup: (),
            translation: position,
            rotation: Quat::IDENTITY,
        }
        .spawn();
    }
}

fn make_cube(pos: Vec2, size: Vec3, dynamic: bool, rng: &mut dyn RngCore) -> EntityId {
    const MASS_MULTIPLIER: f32 = 10.;

    let volume = size.dot(Vec3::ONE);
    Entity::new()
        .with(cube(), ())
        .with(cast_shadows(), ())
        // Properties
        .with(translation(), vec3(pos.x, pos.y, size.z / 2.))
        .with(
            rotation(),
            Quat::from_rotation_x((rng.gen::<f32>() - 0.5) * 5f32.to_radians())
                * Quat::from_rotation_z(rng.gen::<f32>() * TAU),
        )
        .with(scale(), size)
        .with(color(), (rng.gen::<Vec3>() * 0.2).extend(1.))
        // Physics
        .with(physics_controlled(), ())
        .with(cube_collider(), Vec3::ONE)
        .with(self::dynamic(), dynamic)
        .with(mass(), volume * MASS_MULTIPLIER)
        .spawn()
}

#[derive(Debug)]
struct Box {
    pos: Vec2,
    radius: f32,
}

#[derive(Default, Debug)]
struct Grid {
    cells: HashMap<IVec2, Vec<Box>>,
    size: usize,
}
impl Grid {
    const CELL_SIZE: f32 = 4.;

    fn position_to_cell(pos: Vec2) -> IVec2 {
        ivec2(
            (pos.x / Self::CELL_SIZE) as i32,
            (pos.y / Self::CELL_SIZE) as i32,
        )
    }

    fn add(&mut self, pos: Vec2, radius: f32) {
        self.cells
            .entry(Self::position_to_cell(pos))
            .or_default()
            .push(Box { pos, radius });

        self.size += 1;
    }

    fn would_collide(&self, pos: Vec2, radius: f32) -> bool {
        let cell = Self::position_to_cell(pos);
        const PROBE_OFFSETS: [IVec2; 9] = [
            ivec2(0, 0),
            ivec2(-1, 0),
            ivec2(1, 0),
            ivec2(0, -1),
            ivec2(0, 1),
            ivec2(-1, -1),
            ivec2(-1, 1),
            ivec2(1, -1),
            ivec2(1, 1),
        ];

        for offset in PROBE_OFFSETS {
            let cell = cell + offset;
            let Some(boxes) = self.cells.get(&cell) else {
                continue;
            };
            for box_ in boxes {
                if (box_.pos - pos).length_squared() < (box_.radius + radius).powi(2) {
                    return true;
                }
            }
        }

        false
    }

    fn size(&self) -> usize {
        self.size
    }
}