use voxel_core::{BiomeCoord, BiomeType, EdgeDir, ElementId, Material};
use crate::WorldMap;

fn biome_char(bt: BiomeType) -> char {
    match bt {
        BiomeType::Grass => '.',
        BiomeType::Sand  => 's',
        BiomeType::Water => '~',
        BiomeType::Rock  => '#',
    }
}

fn element_char(e: ElementId) -> char {
    match e {
        ElementId::Stone    => 'o',
        ElementId::Sand     => 'S',
        ElementId::Iron     => 'I',
        ElementId::Crystal  => 'C',
        ElementId::Obsidian => 'X',
    }
}

/// Top-down view of layer 6 (surface) for one biome.
/// Characters: `.` grass  `s` sand  `~` water  `#` rock
pub fn ascii_dump(map: &WorldMap, coord: BiomeCoord) -> String {
    let grid = map.biome(coord);
    let bt   = map.biome_type(coord);
    let mut out = String::new();
    out.push_str(&format!(
        "Biome ({},{}) — {:?}\n",
        coord.row, coord.col, bt
    ));
    out.push_str("legend: . grass  s sand  ~ water  # rock\n");
    out.push_str("layer 6 (surface):\n");

    // y=0 is "north" (top of map)
    for vy in 0u8..12 {
        for vx in 0u8..12 {
            let v = grid.get(vx, vy, 5);
            let ch = match v.material {
                Material::Ground(bt) => biome_char(bt),
                _ => '?',
            };
            out.push(ch);
        }
        out.push('\n');
    }

    out.push_str("\nunderground element map (layer 5, z=4):\n");
    out.push_str("legend: . none  o stone  S sand  I iron  C crystal  X obsidian\n");
    for vy in 0u8..12 {
        for vx in 0u8..12 {
            let v = grid.get(vx, vy, 4);
            let ch = match v.element {
                Some(e) => element_char(e),
                None    => '.',
            };
            out.push(ch);
        }
        out.push('\n');
    }

    out
}

/// 6×6 macro grid with connection indicators.
/// `=` horizontal compatible edge, `|` vertical compatible edge, ` ` none.
pub fn ascii_macro(map: &WorldMap) -> String {
    // Each biome cell is rendered as a 3-char-wide block.
    // Between cells, we show a connection character.
    let mut out = String::new();
    out.push_str("6×6 macro map\n");
    out.push_str("legend:  . grass  s sand  ~ water  # rock\n");
    out.push_str("         = compatible H edge  | compatible V edge\n\n");

    for row in 0..6u8 {
        // Biome row
        for col in 0..6u8 {
            let coord = BiomeCoord::new(row, col);
            let bt = map.biome_type(coord);
            out.push('[');
            out.push(biome_char(bt));
            out.push(']');

            // East edge indicator
            if col < 5 {
                let conn = map.connection(coord, EdgeDir::East);
                let ch = match conn {
                    Some(c) if c.compatible => '=',
                    _ => ' ',
                };
                out.push(ch);
            }
        }
        out.push('\n');

        // South edge row
        if row < 5 {
            for col in 0..6u8 {
                let coord = BiomeCoord::new(row, col);
                let conn = map.connection(coord, EdgeDir::South);
                let ch = match conn {
                    Some(c) if c.compatible => '|',
                    _ => ' ',
                };
                out.push(' ');
                out.push(ch);
                out.push(' ');
                if col < 5 { out.push(' '); }
            }
            out.push('\n');
        }
    }

    out
}
