use super::*;

pub struct GenSettings {
    pub seed: u32,
    pub scale: f32,
    pub height: f32,
}

impl Default for GenSettings {
    fn default() -> Self {
        GenSettings {
            seed: 0,
            scale: 0.2,
            height: 0.2,
        }
    }
}

pub fn generate_world(
    gen_settings: &GenSettings,
    blocks: &Vec<CpuOctree>,
) -> Result<CpuOctree, String> {
    let mut octree = CpuOctree::new(0);

    let mut rng = RandomNumberGenerator::new();
    let mut terrain_noise = FastNoise::seeded(gen_settings.seed as u64);
    terrain_noise.set_noise_type(NoiseType::SimplexFractal);
    terrain_noise.set_fractal_type(FractalType::FBM);
    terrain_noise.set_fractal_octaves(5);
    terrain_noise.set_fractal_gain(0.6);
    terrain_noise.set_fractal_lacunarity(2.0);
    terrain_noise.set_frequency(2.0);

    let mut other_noise = FastNoise::seeded(gen_settings.seed as u64);
    other_noise.set_noise_type(NoiseType::SimplexFractal);
    other_noise.set_fractal_type(FractalType::FBM);
    other_noise.set_frequency(2.0);

    let tree_structure = CpuOctree::load_structure("structures/tree.vox".to_string());

    let world_depth = 8;
    let world_size = 1 << world_depth;
    let voxel_size = 2.0 / world_size as f32;
    for x in 0..world_size {
        for z in 0..world_size {
            let mut depth = 0;
            for y in (0..world_size).rev() {
                let mut pos = Vector3::new(x as f32, y as f32, z as f32);
                pos /= world_size as f32 / 2.0;
                pos -= Vector3::new(1.0, 1.0, 1.0);

                let n_pos = pos * gen_settings.scale;
                let mut v = terrain_noise.get_noise3d(n_pos.x, n_pos.y, n_pos.z) + 1.0;

                // Height
                v = v * gen_settings.height;

                // Edge of world
                let edge_distance = 0.5;
                let edge = (-pos.x.abs() + edge_distance)
                    .min(-pos.z.abs() + edge_distance)
                    .min(-pos.y.abs() + 0.0)
                    .min(0.0);
                v = v + edge;

                if v > 0.0 {
                    if depth == 0 {
                        octree.put_in_block(pos, 3, world_depth, blocks);

                        if rng.range(0, 100) == 0 {
                            for voxel in &tree_structure {
                                let tree_pos = Vector3::new(
                                    voxel.0.x as f32,
                                    voxel.0.y as f32,
                                    voxel.0.z as f32,
                                ) * voxel_size;
                                octree.put_in_block(pos + tree_pos, voxel.1, world_depth, blocks);
                            }
                        }
                    } else if depth < 5 {
                        octree.put_in_block(pos, 2, world_depth, blocks);
                    } else {
                        octree.put_in_block(pos, 1, world_depth, blocks);
                    }

                    depth += rng.range(1, 4);
                } else {
                    depth -= rng.range(1, 4);
                    depth = depth.max(0);
                }
            }
        }
    }

    println!("SVO size: {}", octree.nodes.len());

    octree.generate_mip_tree();

    Ok(octree)
}
