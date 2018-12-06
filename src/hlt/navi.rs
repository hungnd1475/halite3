use hlt::direction::Direction;
use hlt::game::Game;
use hlt::position::Position;
use hlt::ship::Ship;
use hlt::ShipId;
use std::collections::HashMap;

#[derive(Debug)]
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

    pub fn is_empty(&self, position: &Position) -> bool {
        self.occupation_at(&position).is_none()
    }

    pub fn is_hostile(&self, position: &Position) -> bool {
        self.occupation_at(&position)
            .map(|occupation| match occupation {
                Occupation::Me(_, _) => false,
                Occupation::Enemy(_) => true,
            }).unwrap_or(false)
    }

    pub fn friend_at(&self, position: &Position) -> Option<ShipId> {
        self.occupation_at(position)
            .map(|occupation| match occupation {
                Occupation::Me(ship_id, _) => Some(*ship_id),
                _ => None,
            }).unwrap_or(None)
    }

    pub fn is_safe(&self, position: &Position) -> bool {
        self.occupation_at(&position)
            .map(|occupation| match *occupation {
                Occupation::Me(ship_id, is_final) => {
                    !is_final && !self.has_ship_waiting_for(&ship_id)
                }
                Occupation::Enemy(_) => true,
            }).unwrap_or(true)
    }

    pub fn occupation_at(&self, position: &Position) -> Option<&Occupation> {
        let position = self.normalize(position);
        self.occupied.get(&position)
    }

    pub fn resolve(&mut self, ship: &Ship, new_position: &Position) {
        if self
            .occupation_at(&ship.position)
            .map(|occupation| match *occupation {
                Occupation::Me(occupied_ship, _) => occupied_ship == ship.id,
                Occupation::Enemy(_) => false,
            }).unwrap_or(false)
        {
            self.mark_safe(&ship.position);
        }
        self.mark_unsafe(new_position, Occupation::Me(ship.id, true));
    }

    fn mark_safe(&mut self, position: &Position) {
        let position = self.normalize(position);
        self.occupied.remove(&position);
    }

    pub fn record_waiting(&mut self, blocking_ship: ShipId, waiting_ship: ShipId) {
        self.waiting.insert(blocking_ship, waiting_ship);
    }

    pub fn has_ship_waiting_for(&self, blocking_ship: &ShipId) -> bool {
        self.waiting.contains_key(blocking_ship)
    }

    pub fn waiting_ship(&self, blocking_ship: &ShipId) -> Option<&ShipId> {
        self.waiting.get(blocking_ship)
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
