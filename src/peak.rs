use hlt::game_map::GameMap;

fn find_local_maxima(map: &GameMap) -> Vec<Position> {
    fn find_row_local_maxima(row: usize, map: &GameMap) -> Vec<Position> {
        let mut maxima = Vec::new();
        let x = row;
        for y in 0..map.height {
            let position = Position { x, y };
            
        }
    }
}
