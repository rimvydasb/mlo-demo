//! Voxel volume → Bevy meshes.
//!
//! One opaque mesh (all solid voxels) plus one translucent water mesh per
//! biome. Both use per-vertex colors (`Mesh::ATTRIBUTE_COLOR`, fully
//! supported by `StandardMaterial` in Bevy 0.19), which lets us bake:
//!
//! - per-voxel color jitter (hashed from world voxel coords — deterministic),
//! - classic 3-neighbour ambient occlusion per vertex,
//!
//! into a single mesh with a single white material, instead of one mesh per
//! flat color.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use voxel_core::BiomeCoord;

use crate::expansion::{voxel_hash01, VoxelKind, VoxelVolume, SUB, VOX};

/// World-space edge length of one voxel (1 cell = 1.0 world unit).
pub const VOXEL_SIZE: f32 = 1.0 / SUB as f32;

pub struct BiomeMeshes {
    pub opaque: Option<Mesh>,
    pub water: Option<Mesh>,
}

/// Voxel-space face table. World mapping is applied at emit time:
/// voxel (x, y, z) → world (x, z, y), i.e. voxel-Z is world-up.
/// `corners` are listed so the emitted world-space winding is CCW from
/// outside (the voxel→world axis swap flips handedness, which this
/// ordering already accounts for).
struct Face {
    normal_v: [i32; 3],
    corners: [[i32; 3]; 4],
}

const FACES: [Face; 6] = [
    // +X (world east)
    Face {
        normal_v: [1, 0, 0],
        corners: [[1, 0, 0], [1, 0, 1], [1, 1, 1], [1, 1, 0]],
    },
    // -X (world west)
    Face {
        normal_v: [-1, 0, 0],
        corners: [[0, 1, 0], [0, 1, 1], [0, 0, 1], [0, 0, 0]],
    },
    // +Y (world south, toward +Z world)
    Face {
        normal_v: [0, 1, 0],
        corners: [[1, 1, 0], [1, 1, 1], [0, 1, 1], [0, 1, 0]],
    },
    // -Y (world north)
    Face {
        normal_v: [0, -1, 0],
        corners: [[0, 0, 0], [0, 0, 1], [1, 0, 1], [1, 0, 0]],
    },
    // +Z (world up)
    Face {
        normal_v: [0, 0, 1],
        corners: [[0, 0, 1], [0, 1, 1], [1, 1, 1], [1, 0, 1]],
    },
    // -Z (world down)
    Face {
        normal_v: [0, 0, -1],
        corners: [[1, 0, 0], [1, 1, 0], [0, 1, 0], [0, 0, 0]],
    },
];

#[derive(Default)]
struct Buffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Buffers {
    fn into_mesh(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
}

pub fn build_biome_meshes(vol: &VoxelVolume, coord: BiomeCoord, seed: u64) -> BiomeMeshes {
    let mut opaque = Buffers::default();
    let mut water = Buffers::default();

    let ox = coord.col as i64 * VOX as i64;
    let oy = coord.row as i64 * VOX as i64;

    for z in 0..VOX {
        for y in 0..VOX {
            for x in 0..VOX {
                let kind = vol.get(x, y, z);
                if kind == VoxelKind::Air {
                    continue;
                }
                let is_water = kind == VoxelKind::Water;

                // Deterministic per-voxel value jitter.
                let h = voxel_hash01(seed, ox + x as i64, oy + y as i64, z as i64);
                let jitter = 1.0 - jitter_amount(kind) * (h - 0.5) * 2.0;
                let base = base_color_linear(kind);
                let rgb = [base[0] * jitter, base[1] * jitter, base[2] * jitter];

                let p = [x as i32, y as i32, z as i32];
                for face in &FACES {
                    let n = face.normal_v;
                    let neighbor = vol.get_or_air(p[0] + n[0], p[1] + n[1], p[2] + n[2]);
                    let visible = if is_water {
                        // Water renders only against air; faces shared with
                        // solids or other water stay hidden.
                        neighbor == VoxelKind::Air
                    } else {
                        !neighbor.is_opaque()
                    };
                    if !visible {
                        continue;
                    }

                    let buf = if is_water { &mut water } else { &mut opaque };
                    emit_face(buf, vol, p, face, rgb, is_water);
                }
            }
        }
    }

    BiomeMeshes {
        opaque: opaque.into_mesh(),
        water: water.into_mesh(),
    }
}

fn emit_face(
    buf: &mut Buffers,
    vol: &VoxelVolume,
    p: [i32; 3],
    face: &Face,
    rgb: [f32; 3],
    is_water: bool,
) {
    let n = face.normal_v;
    // Voxel-space tangent axes of this face's plane.
    let axis = n.iter().position(|&c| c != 0).unwrap();
    let (u_axis, v_axis) = ((axis + 1) % 3, (axis + 2) % 3);

    let base = buf.positions.len() as u32;
    let mut ao = [1.0f32; 4];

    for (i, corner) in face.corners.iter().enumerate() {
        // World position: voxel (x, y, z) → world (x, z, y), scaled.
        let vx = (p[0] + corner[0]) as f32;
        let vy = (p[1] + corner[1]) as f32;
        let vz = (p[2] + corner[2]) as f32;
        buf.positions
            .push([vx * VOXEL_SIZE, vz * VOXEL_SIZE, vy * VOXEL_SIZE]);
        buf.normals.push([n[0] as f32, n[2] as f32, n[1] as f32]);

        let occ = if is_water {
            1.0
        } else {
            corner_ao(vol, p, n, axis, u_axis, v_axis, *corner)
        };
        ao[i] = occ;
        let alpha = 1.0;
        buf.colors
            .push([rgb[0] * occ, rgb[1] * occ, rgb[2] * occ, alpha]);
    }

    // Flip the quad diagonal where it makes AO interpolate more smoothly.
    if ao[0] + ao[2] >= ao[1] + ao[3] {
        buf.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        buf.indices
            .extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
    }
}

/// Classic voxel AO: for a face corner, occlusion from the two edge
/// neighbours and the diagonal neighbour in the plane one step along the
/// face normal.
fn corner_ao(
    vol: &VoxelVolume,
    p: [i32; 3],
    n: [i32; 3],
    axis: usize,
    u_axis: usize,
    v_axis: usize,
    corner: [i32; 3],
) -> f32 {
    // Corner offsets 0/1 → neighbour direction -1/+1 on each tangent axis.
    let su = corner[u_axis] * 2 - 1;
    let sv = corner[v_axis] * 2 - 1;

    let mut side1 = p;
    side1[axis] += n[axis];
    let mut side2 = side1;
    let mut diag = side1;
    side1[u_axis] += su;
    side2[v_axis] += sv;
    diag[u_axis] += su;
    diag[v_axis] += sv;

    let occludes = |q: [i32; 3]| vol.get_or_air(q[0], q[1], q[2]).is_opaque();
    let (s1, s2) = (occludes(side1), occludes(side2));
    let level = if s1 && s2 {
        3
    } else {
        s1 as u8 + s2 as u8 + occludes(diag) as u8
    };
    [1.0, 0.82, 0.66, 0.5][level as usize]
}

fn jitter_amount(kind: VoxelKind) -> f32 {
    match kind {
        VoxelKind::Air => 0.0,
        VoxelKind::Grass => 0.13,
        VoxelKind::Dirt => 0.10,
        VoxelKind::Sand => 0.06,
        VoxelKind::Water => 0.04,
        VoxelKind::Stone => 0.14,
        VoxelKind::Gold => 0.18,
        VoxelKind::Iron => 0.14,
        VoxelKind::Snow => 0.03,
    }
}

/// Palette in sRGB, tuned against docs/reference/*.jpg. The inspector's
/// preview panel reads this directly; the mesher converts to linear.
pub fn base_color_srgb(kind: VoxelKind) -> [f32; 3] {
    match kind {
        VoxelKind::Air => [0.0, 0.0, 0.0],
        VoxelKind::Grass => [0.36, 0.70, 0.22],
        VoxelKind::Dirt => [0.56, 0.36, 0.22],
        VoxelKind::Sand => [0.89, 0.80, 0.55],
        VoxelKind::Water => [0.16, 0.52, 0.80],
        VoxelKind::Stone => [0.37, 0.37, 0.39],
        VoxelKind::Gold => [0.90, 0.73, 0.22],
        VoxelKind::Iron => [0.68, 0.39, 0.30],
        VoxelKind::Snow => [0.94, 0.96, 0.99],
    }
}

/// Same palette in linear space — vertex colors multiply the material in
/// linear space.
pub fn base_color_linear(kind: VoxelKind) -> [f32; 3] {
    let [r, g, b] = base_color_srgb(kind);
    let lin = Color::srgb(r, g, b).to_linear();
    [lin.red, lin.green, lin.blue]
}
