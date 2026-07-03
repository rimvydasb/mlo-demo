//! Cursor → cell resolution.
//!
//! Picking resolves to a *cell* (the gameplay unit), not a voxel: an
//! Amanatides–Woo DDA walks the 8×8×12 cell grid in world units (1 cell =
//! 1 world unit; world Y is cell Z).

use bevy::prelude::*;
use voxel_core::{CellGrid, CellType, CELLS_XY, CELLS_Z};

/// First non-air cell with z < cutoff along the ray, or None.
pub fn raycast_cell(ray: Ray3d, grid: &CellGrid, cutoff: u8) -> Option<(u8, u8, u8)> {
    // Cell-space bounds per axis: x, y are the 8-cell footprint, z (up) is
    // the 12-layer column.
    let bounds = Vec3::new(CELLS_XY as f32, CELLS_XY as f32, CELLS_Z as f32);

    // World (x, y, z) → cell space (x, z, y): world Y is cell Z (up).
    let origin = Vec3::new(ray.origin.x, ray.origin.z, ray.origin.y);
    let dir = Vec3::new(ray.direction.x, ray.direction.z, ray.direction.y);

    // Clip the ray to the grid's bounding box with the slab method.
    let mut t_enter = 0.0_f32;
    let mut t_exit = f32::INFINITY;
    for i in 0..3 {
        if dir[i].abs() < 1e-8 {
            if origin[i] < 0.0 || origin[i] > bounds[i] {
                return None;
            }
            continue;
        }
        let t0 = (0.0 - origin[i]) / dir[i];
        let t1 = (bounds[i] - origin[i]) / dir[i];
        t_enter = t_enter.max(t0.min(t1));
        t_exit = t_exit.min(t0.max(t1));
    }
    if t_enter > t_exit {
        return None;
    }

    let start = origin + dir * (t_enter + 1e-4);
    let max_cell = IVec3::new(CELLS_XY as i32 - 1, CELLS_XY as i32 - 1, CELLS_Z as i32 - 1);
    let mut cell = start.floor().as_ivec3().clamp(IVec3::ZERO, max_cell);

    let step = IVec3::new(
        dir.x.signum() as i32,
        dir.y.signum() as i32,
        dir.z.signum() as i32,
    );
    // Distance along the ray to the next grid plane per axis, and the
    // per-cell increment.
    let mut t_max = Vec3::ZERO;
    let mut t_delta = Vec3::ZERO;
    for i in 0..3 {
        if dir[i].abs() < 1e-8 {
            t_max[i] = f32::INFINITY;
            t_delta[i] = f32::INFINITY;
        } else {
            let next_plane = if dir[i] > 0.0 {
                cell[i] as f32 + 1.0
            } else {
                cell[i] as f32
            };
            t_max[i] = (next_plane - start[i]) / dir[i];
            t_delta[i] = 1.0 / dir[i].abs();
        }
    }

    loop {
        let (x, y, z) = (cell.x, cell.y, cell.z);
        if x < 0
            || y < 0
            || z < 0
            || x >= CELLS_XY as i32
            || y >= CELLS_XY as i32
            || z >= CELLS_Z as i32
        {
            return None;
        }
        if (z as u8) < cutoff && grid.get(x as u8, y as u8, z as u8) != CellType::Air {
            return Some((x as u8, y as u8, z as u8));
        }

        // Step to the next cell across the nearest plane.
        if t_max.x <= t_max.y && t_max.x <= t_max.z {
            cell.x += step.x;
            t_max.x += t_delta.x;
        } else if t_max.y <= t_max.z {
            cell.y += step.y;
            t_max.y += t_delta.y;
        } else {
            cell.z += step.z;
            t_max.z += t_delta.z;
        }
    }
}
