//! ASCII dumps for headless inspection and snapshot tests.

use crate::WorldMap;
use voxel_core::{BiomeCoord, BiomeType, CellType, EdgeDir, CELLS_XY, CELLS_Z, SURFACE_Z};

fn biome_char(bt: BiomeType) -> char {
    match bt {
        BiomeType::Grass => '.',
        BiomeType::Sand => 's',
        BiomeType::Water => '~',
        BiomeType::Winter => '*',
    }
}

fn cell_char(c: CellType) -> char {
    match c {
        CellType::Air => ' ',
        CellType::Soil => '.',
        CellType::Sand => 's',
        CellType::Water => '~',
        CellType::Stone => '#',
        CellType::Gold => 'G',
        CellType::Iron => 'I',
    }
}

const LEGEND: &str = "legend: (space) air  . soil  s sand  ~ water  # stone  G gold  I iron";

/// Full dump of one biome: surface map, relief height map, and every
/// underground layer. y=0 is north (top of the map).
pub fn ascii_dump(map: &WorldMap, coord: BiomeCoord) -> String {
    let grid = map.biome(coord);
    let bt = map.biome_type(coord);
    let max = CELLS_XY as u8;
    let mut out = String::new();

    out.push_str(&format!(
        "Biome ({},{}) — {}\n{LEGEND}\n\n",
        coord.row,
        coord.col,
        bt.label()
    ));

    out.push_str("surface (cell layer 6):\n");
    for y in 0..max {
        for x in 0..max {
            out.push(cell_char(grid.get(x, y, SURFACE_Z)));
        }
        out.push('\n');
    }

    out.push_str("\nrelief height (cells above surface, 0-6):\n");
    for y in 0..max {
        for x in 0..max {
            let h = (SURFACE_Z + 1..CELLS_Z as u8)
                .take_while(|&z| grid.get(x, y, z) != CellType::Air)
                .count();
            out.push(char::from_digit(h as u32, 10).unwrap());
        }
        out.push('\n');
    }

    for z in (0..SURFACE_Z).rev() {
        out.push_str(&format!("\nunderground (cell layer {}):\n", z + 1));
        for y in 0..max {
            for x in 0..max {
                out.push(cell_char(grid.get(x, y, z)));
            }
            out.push('\n');
        }
    }

    out
}

/// 6×6 macro grid with connection indicators.
/// `=` compatible horizontal edge, `|` compatible vertical edge.
pub fn ascii_macro(map: &WorldMap) -> String {
    let mut out = String::new();
    out.push_str("6×6 macro map\n");
    out.push_str("legend:  . grass  s sand  ~ water  * winter\n");
    out.push_str("         = compatible H edge  | compatible V edge\n\n");

    for row in 0..6u8 {
        for col in 0..6u8 {
            let coord = BiomeCoord::new(row, col);
            out.push('[');
            out.push(biome_char(map.biome_type(coord)));
            out.push(']');

            if col < 5 {
                let ch = match map.connection(coord, EdgeDir::East) {
                    Some(c) if c.compatible => '=',
                    _ => ' ',
                };
                out.push(ch);
            }
        }
        out.push('\n');

        if row < 5 {
            for col in 0..6u8 {
                let coord = BiomeCoord::new(row, col);
                let ch = match map.connection(coord, EdgeDir::South) {
                    Some(c) if c.compatible => '|',
                    _ => ' ',
                };
                out.push(' ');
                out.push(ch);
                out.push(' ');
                if col < 5 {
                    out.push(' ');
                }
            }
            out.push('\n');
        }
    }

    out
}
