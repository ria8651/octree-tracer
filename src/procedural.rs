use super::*;

pub struct GenSettings {
    pub seed: u32,
    pub scale: f32,
    pub height: f32,
}

impl Default for GenSettings {
    fn default() -> Self {
        GenSettings  {
            seed: 0,
            scale: 0.2,
            height: 0.2,
        }
    }
}

pub fn generate_world(
    noise_settings: &GenSettings,
    blocks: &Vec<CpuOctree>,
) -> Result<CpuOctree, String> {
    let mut octree = CpuOctree::new(0);

    let mut noise = FastNoise::seeded(noise_settings.seed as u64);
    noise.set_noise_type(NoiseType::SimplexFractal);
    noise.set_fractal_type(FractalType::FBM);
    noise.set_fractal_octaves(5);
    noise.set_fractal_gain(0.6);
    noise.set_fractal_lacunarity(2.0);
    noise.set_frequency(2.0);

    let world_depth = 8;
    let world_size = 1 << world_depth;
    for x in 0..world_size {
        for z in 0..world_size {
            for y in 0..world_size {
                let mut pos = Vector3::new(x as f32, y as f32, z as f32);
                pos /= world_size as f32 / 2.0;
                pos -= Vector3::new(1.0, 1.0, 1.0);

                let n_pos = pos * noise_settings.scale;
                let v = noise.get_noise3d(n_pos.x, n_pos.y, n_pos.z) * noise_settings.height - pos.y;

                if v > 0.0 {
                    octree.put_in_block(pos, 3, world_depth, blocks);
                }
            }
        }
    }

    println!("SVO size: {}", octree.nodes.len());

    octree.generate_mip_tree();

    Ok(octree)
}
