use serde::{Deserialize, Serialize};

pub type Seed = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum BiomeType {
    Grass,
    Sand,
    Water,
    Rock,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum ElementId {
    Stone,
    Sand,
    Iron,
    Crystal,
    Obsidian,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Material {
    #[default]
    Air,
    Ground(BiomeType),
    Underground,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Voxel {
    pub material: Material,
    pub element: Option<ElementId>,
}

pub struct VoxelGrid {
    // x + 12*y + 144*z; z=0 is layer 1 (bottom underground)
    voxels: Box<[Voxel; 1728]>,
}

impl VoxelGrid {
    pub fn new() -> Self {
        Self {
            voxels: Box::new([Voxel::default(); 1728]),
        }
    }

    #[inline]
    fn idx(x: u8, y: u8, z: u8) -> usize {
        x as usize + 12 * y as usize + 144 * z as usize
    }

    pub fn get(&self, x: u8, y: u8, z: u8) -> Voxel {
        self.voxels[Self::idx(x, y, z)]
    }

    pub fn set(&mut self, x: u8, y: u8, z: u8, v: Voxel) {
        self.voxels[Self::idx(x, y, z)] = v;
    }

    pub fn get_or_air(&self, x: i32, y: i32, z: i32) -> Voxel {
        if x < 0 || y < 0 || z < 0 || x >= 12 || y >= 12 || z >= 12 {
            Voxel::default()
        } else {
            self.get(x as u8, y as u8, z as u8)
        }
    }
}

impl Default for VoxelGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for VoxelGrid {
    fn clone(&self) -> Self {
        Self {
            voxels: self.voxels.clone(),
        }
    }
}

impl PartialEq for VoxelGrid {
    fn eq(&self, other: &Self) -> bool {
        self.voxels.as_ref() == other.voxels.as_ref()
    }
}

impl Eq for VoxelGrid {}

impl std::fmt::Debug for VoxelGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxelGrid").finish_non_exhaustive()
    }
}

// ── Coordinates ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BiomeCoord {
    pub row: u8,
    pub col: u8,
}

impl BiomeCoord {
    pub fn new(row: u8, col: u8) -> Self {
        Self { row, col }
    }

    pub fn neighbor(self, dir: EdgeDir) -> Option<BiomeCoord> {
        match dir {
            EdgeDir::North if self.row > 0 => Some(Self::new(self.row - 1, self.col)),
            EdgeDir::South if self.row < 5 => Some(Self::new(self.row + 1, self.col)),
            EdgeDir::West if self.col > 0 => Some(Self::new(self.row, self.col - 1)),
            EdgeDir::East if self.col < 5 => Some(Self::new(self.row, self.col + 1)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EdgeDir {
    North,
    South,
    East,
    West,
}

impl EdgeDir {
    pub fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::South => Self::North,
            Self::East => Self::West,
            Self::West => Self::East,
        }
    }

    pub fn all() -> [EdgeDir; 4] {
        [Self::North, Self::South, Self::East, Self::West]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Connection {
    pub compatible: bool,
    pub surface: BiomeType,
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voxel_default_is_air() {
        assert_eq!(Voxel::default().material, Material::Air);
        assert_eq!(Voxel::default().element, None);
    }

    #[test]
    fn voxel_grid_round_trip() {
        let mut g = VoxelGrid::new();
        g.set(3, 7, 11, Voxel { material: Material::Underground, element: Some(ElementId::Iron) });
        let v = g.get(3, 7, 11);
        assert_eq!(v.material, Material::Underground);
        assert_eq!(v.element, Some(ElementId::Iron));
    }

    #[test]
    fn biome_coord_neighbor() {
        let c = BiomeCoord::new(3, 3);
        assert_eq!(c.neighbor(EdgeDir::North), Some(BiomeCoord::new(2, 3)));
        assert_eq!(c.neighbor(EdgeDir::South), Some(BiomeCoord::new(4, 3)));
        assert_eq!(BiomeCoord::new(0, 0).neighbor(EdgeDir::North), None);
        assert_eq!(BiomeCoord::new(5, 5).neighbor(EdgeDir::East), None);
    }
}
