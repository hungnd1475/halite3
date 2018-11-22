use hlt::direction::Direction;
use hlt::game::Game;
use hlt::position::Position;
use hlt::ShipId;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub enum Occupation {
    Me(ShipId, bool),
    Enemy(ShipId),
}

pub struct Navi {
    pub width: usize,
    pub height: usize,
    pub occupied: HashMap<Position, Occupation>,
    pub waiting: HashMap<ShipId, ShipId>,
}

impl Navi {
    pub fn new(width: usize, height: usize) -> Navi {
        Navi {
            width,
            height,
            occupied: HashMap::new(),
            waiting: HashMap::new(),
        }
    }

    pub fn update_frame(&mut self, game: &Game) {
        self.clear();
        let my_id = &game.my_id;

        for player in &game.players {
            for ship_id in &player.ship_ids {
                let ship = &game.ships[ship_id];
                if *my_id == ship.owner {
                    self.mark_unsafe(&ship.position, Occupation::Me(ship.id, false));
                } else {
                    self.mark_unsafe(&ship.position, Occupation::Enemy(ship.id));
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.occupied.drain();
        self.waiting.drain();
    }

    pub fn is_safe(&self, position: &Position) -> bool {
        let position = self.normalize(position);
        let occupation = self.occupation_at(&position);
        occupation.is_none()
    }

    pub fn occupation_at(&self, position: &Position) -> Option<&Occupation> {
        let position = self.normalize(position);
        self.occupied.get(&position)
    }

    pub fn resolve(&mut self, position: &Position, ship_id: ShipId) {
        self.mark_unsafe(position, Occupation::Me(ship_id, true));
    }

    pub fn mark_safe(&mut self, position: &Position) {
        let position = self.normalize(position);
        self.occupied.remove(&position);
    }

    pub fn record_waiting(&mut self, blocking_ship: ShipId, waiting_ship: ShipId) {
        self.waiting.insert(blocking_ship, waiting_ship);
    }

    pub fn has_ship_waiting_for(&self, blocking_ship: &ShipId) -> bool {
        self.waiting.contains_key(blocking_ship)
    }

    pub fn get_unsafe_moves(&self, source: &Position, destination: &Position) -> Vec<Direction> {
        let normalized_source = self.normalize(source);
        let normalized_destination = self.normalize(destination);

        let dx = (normalized_source.x - normalized_destination.x).abs() as usize;
        let dy = (normalized_source.y - normalized_destination.y).abs() as usize;

        let wrapped_dx = self.width - dx;
        let wrapped_dy = self.height - dy;

        let mut possible_moves: Vec<Direction> = Vec::new();

        if normalized_source.x < normalized_destination.x {
            possible_moves.push(if dx > wrapped_dx {
                Direction::West
            } else {
                Direction::East
            });
        } else if normalized_source.x > normalized_destination.x {
            possible_moves.push(if dx < wrapped_dx {
                Direction::West
            } else {
                Direction::East
            });
        }

        if normalized_source.y < normalized_destination.y {
            possible_moves.push(if dy > wrapped_dy {
                Direction::North
            } else {
                Direction::South
            });
        } else if normalized_source.y > normalized_destination.y {
            possible_moves.push(if dy < wrapped_dy {
                Direction::North
            } else {
                Direction::South
            });
        }

        possible_moves
    }

    pub fn normalize(&self, position: &Position) -> Position {
        let width = self.width as i32;
        let height = self.height as i32;
        let x = ((position.x % width) + width) % width;
        let y = ((position.y % height) + height) % height;
        Position { x, y }
    }

    fn mark_unsafe(&mut self, position: &Position, occupation: Occupation) {
        let position = self.normalize(position);
        self.occupied.insert(position, occupation);
    }
}
