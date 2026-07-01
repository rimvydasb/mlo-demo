use noise::{NoiseFn, Perlin};
use voxel_core::{BiomeCoord, BiomeType, ElementId, Material, Seed, Voxel, VoxelGrid};

fn derive_u32(seed: Seed, offset: u64) -> u32 {
    let mixed = seed
        .wrapping_add(offset.wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_mul(0x517cc1b727220a95);
    (mixed ^ (mixed >> 32)) as u32
}

struct ElementNoises {
    stone:    Perlin,
    sand:     Perlin,
    iron:     Perlin,
    crystal:  Perlin,
    obsidian: Perlin,
    height:   Perlin,
}

impl ElementNoises {
    fn new(seed: Seed) -> Self {
        Self {
            stone:    Perlin::new(derive_u32(seed, 1)),
            sand:     Perlin::new(derive_u32(seed, 2)),
            iron:     Perlin::new(derive_u32(seed, 3)),
            crystal:  Perlin::new(derive_u32(seed, 4)),
            obsidian: Perlin::new(derive_u32(seed, 5)),
            height:   Perlin::new(derive_u32(seed, 6)),
        }
    }
}

pub fn generate_biome(
    seed: Seed,
    coord: BiomeCoord,
    biome_type: BiomeType,
    _north: Option<BiomeType>,
    _south: Option<BiomeType>,
    _west: Option<BiomeType>,
    _east: Option<BiomeType>,
) -> VoxelGrid {
    let noise = ElementNoises::new(seed);
    let mut grid = VoxelGrid::new();

    let ox = coord.col as f64 * 12.0;
    let oy = coord.row as f64 * 12.0;

    for vy in 0u8..12 {
        for vx in 0u8..12 {
            let wx = ox + vx as f64;
            let wy = oy + vy as f64;
            let is_edge = vx == 0 || vx == 11 || vy == 0 || vy == 11;

            // Underground z=0..4  (layers 1–5)
            for vz in 0u8..5 {
                let element = pick_element(vz, wx, wy, &noise);
                grid.set(vx, vy, vz, Voxel { material: Material::Underground, element });
            }

            // Surface z=5  (layer 6) — always solid, typed
            grid.set(vx, vy, 5, Voxel { material: Material::Ground(biome_type), element: None });

            // Relief z=6..11 (layers 7–12)
            let relief = if is_edge || biome_type == BiomeType::Water {
                0u8
            } else {
                let h = noise.height.get([wx * 0.07, wy * 0.07]); // −1..1
                ((h + 1.0) * 3.0).clamp(0.0, 6.0) as u8
            };

            for vz in 6u8..12 {
                if (vz - 6) < relief {
                    grid.set(vx, vy, vz, Voxel { material: Material::Ground(biome_type), element: None });
                }
            }
        }
    }

    grid
}

fn pick_element(vz: u8, wx: f64, wy: f64, n: &ElementNoises) -> Option<ElementId> {
    // "Deep" = low z (layer 1 = z 0).  Rarer elements go deeper.
    let s = |noise: &Perlin, scale: f64| -> f64 {
        noise.get([wx * scale, wy * scale, vz as f64 * 0.4])
    };

    if vz <= 1 && s(&n.obsidian, 0.15) > 0.80 { return Some(ElementId::Obsidian); }
    if vz <= 2 && s(&n.crystal,  0.12) > 0.65 { return Some(ElementId::Crystal);  }
    if              s(&n.iron,    0.10) > 0.40 { return Some(ElementId::Iron);     }
    if vz >= 1 && s(&n.sand,     0.10) > 0.10 { return Some(ElementId::Sand);     }
    if vz >= 2 && s(&n.stone,    0.08) > -0.30 { return Some(ElementId::Stone);   }

    None
}
