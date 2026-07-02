//! Cell-tier world model shared by every crate.
//!
//! The world is a 6×6 grid of biomes; a biome is a 12×12×12 grid of *cells*.
//! Cells are the logical/gameplay unit. The purely cosmetic cell → 4×4×4
//! voxel expansion lives in `voxel-render` and never leaks into this crate.

use serde::{Deserialize, Serialize};

pub type Seed = u64;

/// Cells per biome edge (biome = CELLS³ cells).
pub const CELLS: usize = 12;
/// Z index of the surface layer (cell layer 6, 1-based).
pub const SURFACE_Z: u8 = 5;
/// First relief Z index eligible for a cosmetic snow cap (cell layer 11).
pub const SNOW_Z: u8 = 10;
/// Biomes per world edge (world = WORLD_BIOMES² biomes).
pub const WORLD_BIOMES: usize = 6;

// ── Cell tier ────────────────────────────────────────────────────────────────

/// Logical cell content. A cell *is* its resource where one exists
/// (stone/gold/iron/water); there is no separate element field.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash, Serialize, Deserialize)]
pub enum CellType {
    #[default]
    Air,
    /// Dirt below ground; grows a grass top when exposed to air.
    Soil,
    Sand,
    Water,
    Stone,
    Gold,
    Iron,
}

/// Minable resource carried by a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum Resource {
    Water,
    Stone,
    Gold,
    Iron,
}

impl CellType {
    pub fn resource(self) -> Option<Resource> {
        match self {
            CellType::Water => Some(Resource::Water),
            CellType::Stone => Some(Resource::Stone),
            CellType::Gold => Some(Resource::Gold),
            CellType::Iron => Some(Resource::Iron),
            CellType::Air | CellType::Soil | CellType::Sand => None,
        }
    }

    /// Solid for support/occlusion purposes (water is not).
    pub fn is_solid(self) -> bool {
        !matches!(self, CellType::Air | CellType::Water)
    }

    pub fn label(self) -> &'static str {
        match self {
            CellType::Air => "Air",
            CellType::Soil => "Soil",
            CellType::Sand => "Sand",
            CellType::Water => "Water",
            CellType::Stone => "Stone",
            CellType::Gold => "Gold",
            CellType::Iron => "Iron",
        }
    }
}

/// 12×12×12 grid of cells. Z is up: z=0 is the deepest underground layer
/// (cell layer 1), z=SURFACE_Z is the surface, z=6..=11 is relief.
#[derive(Clone, PartialEq, Eq)]
pub struct CellGrid {
    cells: Box<[CellType; CELLS * CELLS * CELLS]>,
}

impl CellGrid {
    pub fn new() -> Self {
        Self {
            cells: Box::new([CellType::Air; CELLS * CELLS * CELLS]),
        }
    }

    #[inline]
    fn idx(x: u8, y: u8, z: u8) -> usize {
        debug_assert!((x as usize) < CELLS && (y as usize) < CELLS && (z as usize) < CELLS);
        x as usize + CELLS * y as usize + CELLS * CELLS * z as usize
    }

    #[inline]
    pub fn get(&self, x: u8, y: u8, z: u8) -> CellType {
        self.cells[Self::idx(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: u8, y: u8, z: u8, c: CellType) {
        self.cells[Self::idx(x, y, z)] = c;
    }

    /// Out-of-bounds reads as Air.
    #[inline]
    pub fn get_or_air(&self, x: i32, y: i32, z: i32) -> CellType {
        let max = CELLS as i32;
        if x < 0 || y < 0 || z < 0 || x >= max || y >= max || z >= max {
            CellType::Air
        } else {
            self.get(x as u8, y as u8, z as u8)
        }
    }
}

impl Default for CellGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CellGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellGrid").finish_non_exhaustive()
    }
}

// ── Macro tier ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum BiomeType {
    Grass,
    Sand,
    Water,
    Rock,
}

impl BiomeType {
    /// The cell type this biome's surface layer is made of.
    pub fn surface_cell(self) -> CellType {
        match self {
            BiomeType::Grass => CellType::Soil,
            BiomeType::Sand => CellType::Sand,
            BiomeType::Water => CellType::Water,
            BiomeType::Rock => CellType::Stone,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BiomeType::Grass => "Grass",
            BiomeType::Sand => "Sand",
            BiomeType::Water => "Water",
            BiomeType::Rock => "Rock",
        }
    }
}

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
        let max = WORLD_BIOMES as u8 - 1;
        match dir {
            EdgeDir::North if self.row > 0 => Some(Self::new(self.row - 1, self.col)),
            EdgeDir::South if self.row < max => Some(Self::new(self.row + 1, self.col)),
            EdgeDir::West if self.col > 0 => Some(Self::new(self.row, self.col - 1)),
            EdgeDir::East if self.col < max => Some(Self::new(self.row, self.col + 1)),
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

/// Edge relation between two adjacent biomes. Compatible edges are flat,
/// share the same surface type, and (later) allow armies across.
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
    fn cell_grid_round_trip() {
        let mut g = CellGrid::new();
        assert_eq!(g.get(3, 7, 11), CellType::Air);
        g.set(3, 7, 11, CellType::Gold);
        assert_eq!(g.get(3, 7, 11), CellType::Gold);
    }

    #[test]
    fn out_of_bounds_is_air() {
        let g = CellGrid::new();
        assert_eq!(g.get_or_air(-1, 0, 0), CellType::Air);
        assert_eq!(g.get_or_air(0, 12, 0), CellType::Air);
    }

    #[test]
    fn resources_map() {
        assert_eq!(CellType::Gold.resource(), Some(Resource::Gold));
        assert_eq!(CellType::Soil.resource(), None);
        assert!(!CellType::Water.is_solid());
        assert!(CellType::Iron.is_solid());
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
