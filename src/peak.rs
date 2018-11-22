use hlt::game_map::GameMap;

#[derive(Copy, Clone)]
enum BufferClass {
    Max,
    Min,
}

enum PeakProcess {
    Maximum,
    BufferBegin(BufferClass),
    BufferEnd(BufferClass),
}

fn process_window(row: i32, col: i32, map: &GameMap) -> Option<PeakProcess> {
    let c1 = map.entity_at(&Position { x: row, y: col - 1 }).halite;
    let c2 = map.entity_at(&Position { x: row, y: col }).halite;
    let c3 = map.entity_at(&Position { x: row, y: col + 1 }).halite;
    if c1 < c2 && c2 > c3 {
        Some(PeakProcess::Maximum)
    } else if c1 == c2 && c2 > c3 {
        Some(PeakProcess::BufferEnd(BufferClass::Max))
    } else if c1 == c2 && c2 < c3 {
        Some(PeakProcess::BufferEnd(BufferClass::Min))
    } else if c1 > c2 && c2 == c3 {
        Some(PeakProcess::BufferStart(BufferClass::Min))
    } else if c1 < c2 && c2 == c3 {
        Some(PeakProcess::BufferStart(BufferClass::Max))
    } else {
        None
    }
}

pub fn find_peaks(map: &GameMap) {
    let mut peaks = vec![];
    for i in 0..map.height {
        let mut buffer_start = None;
        for j in 0..map.width {
            let result = process_window(i, j, map);
            match result {
                PeakProcess::Maximum => peaks.push((i, j)),
                PeakProcess::BufferStart(class) => buffer_start = Some(class),
                PeakProcess::BufferEnd(class) => {
                    let buffer_start = buffer_start.unwrap();
                    if buffer_start == BufferClass::Max && class == BufferClass::Max {
                        peaks.push((i, j));
                    }
                }
            }
        }
    }
}
