use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use voxel_core::{BiomeType, ElementId, Material, Voxel, VoxelGrid};
use std::collections::HashMap;

// Face definitions: (voxel-space neighbor offset, world-space normal, 4 world-space vertex offsets)
// World mapping: voxel (vx,vy,vz) → world (vx, vz, vy) — voxel-Z is world-Y (up)
const FACE_DEFS: &[([i32; 3], [f32; 3], [[f32; 3]; 4])] = &[
    ([1, 0, 0],  [1., 0., 0.],  [[1.,0.,0.],[1.,0.,1.],[1.,1.,1.],[1.,1.,0.]]), // +X east
    ([-1,0, 0],  [-1.,0., 0.],  [[0.,0.,1.],[0.,0.,0.],[0.,1.,0.],[0.,1.,1.]]), // -X west
    ([0, 0, 1],  [0., 1., 0.],  [[0.,1.,0.],[0.,1.,1.],[1.,1.,1.],[1.,1.,0.]]), // +Y top (dvz=+1)
    ([0, 0,-1],  [0.,-1., 0.],  [[1.,0.,1.],[0.,0.,1.],[0.,0.,0.],[1.,0.,0.]]), // -Y bottom
    ([0, 1, 0],  [0., 0., 1.],  [[1.,0.,1.],[0.,0.,1.],[0.,1.,1.],[1.,1.,1.]]), // +Z south (dvy=+1)
    ([0,-1, 0],  [0., 0.,-1.],  [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.]]), // -Z north
];

type GroupData = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);

/// Build one mesh per unique voxel color; returns (Mesh, [r, g, b]) pairs.
/// Caller creates one StandardMaterial per pair using Color::srgb(r, g, b).
pub fn build_voxel_meshes(grid: &VoxelGrid, layer_cutoff: u8) -> Vec<(Mesh, [f32; 3])> {
    // key: bit-pattern of rgb floats; value: (rgb, geometry buffers)
    let mut groups: HashMap<[u8; 12], ([f32; 3], GroupData)> = HashMap::new();

    for vz in 0u8..12 {
        if vz >= layer_cutoff { continue; }
        for vy in 0u8..12 {
            for vx in 0u8..12 {
                let voxel = grid.get(vx, vy, vz);
                if voxel.material == Material::Air { continue; }

                let wx = vx as f32;
                let wy = vz as f32; // voxel-Z → world-Y (up)
                let wz = vy as f32; // voxel-Y → world-Z

                let [r, g, b, _] = voxel_color(voxel);
                let key = color_key([r, g, b]);

                for &([dvx, dvy, dvz], normal, vert_offs) in FACE_DEFS {
                    let nx = vx as i32 + dvx;
                    let ny = vy as i32 + dvy;
                    let nz = vz as i32 + dvz;

                    let nb_air = if nx < 0 || ny < 0 || nz < 0
                                    || nx >= 12 || ny >= 12 || nz >= 12
                    {
                        true
                    } else {
                        let nb = grid.get(nx as u8, ny as u8, nz as u8);
                        nb.material == Material::Air || (nz as u8) >= layer_cutoff
                    };
                    if !nb_air { continue; }

                    let (_, (positions, normals, uvs, indices)) = groups
                        .entry(key)
                        .or_insert_with(|| ([r, g, b], (vec![], vec![], vec![], vec![])));

                    let base = positions.len() as u32;
                    for [dx, dy, dz] in vert_offs {
                        positions.push([wx + dx, wy + dy, wz + dz]);
                        normals.push(normal);
                        uvs.push([dx, dz]);
                    }
                    indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
                }
            }
        }
    }

    groups.into_values().map(|(color, (positions, normals, uvs, indices))| {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));
        (mesh, color)
    }).collect()
}

fn color_key(rgb: [f32; 3]) -> [u8; 12] {
    let mut key = [0u8; 12];
    for (i, f) in rgb.iter().enumerate() {
        key[i*4..(i+1)*4].copy_from_slice(&f.to_bits().to_le_bytes());
    }
    key
}

pub fn proxy_material_color(bt: BiomeType) -> LinearRgba {
    let (r, g, b) = match bt {
        BiomeType::Grass => (0.15, 0.45, 0.15),
        BiomeType::Sand  => (0.60, 0.55, 0.35),
        BiomeType::Water => (0.10, 0.25, 0.60),
        BiomeType::Rock  => (0.35, 0.35, 0.35),
    };
    LinearRgba::new(r, g, b, 1.0)
}

fn voxel_color(voxel: Voxel) -> [f32; 4] {
    if let Some(e) = voxel.element {
        return element_color(e);
    }
    match voxel.material {
        Material::Air                          => [0., 0., 0., 0.],
        Material::Ground(BiomeType::Grass)     => [0.22, 0.68, 0.22, 1.],
        Material::Ground(BiomeType::Sand)      => [0.88, 0.78, 0.50, 1.],
        Material::Ground(BiomeType::Water)     => [0.12, 0.42, 0.88, 1.],
        Material::Ground(BiomeType::Rock)      => [0.52, 0.52, 0.52, 1.],
        Material::Underground                  => [0.33, 0.22, 0.14, 1.],
    }
}

fn element_color(e: ElementId) -> [f32; 4] {
    match e {
        ElementId::Stone    => [0.60, 0.60, 0.60, 1.],
        ElementId::Sand     => [0.92, 0.84, 0.58, 1.],
        ElementId::Iron     => [0.72, 0.40, 0.20, 1.],
        ElementId::Crystal  => [0.42, 0.82, 0.92, 1.],
        ElementId::Obsidian => [0.12, 0.06, 0.18, 1.],
    }
}
