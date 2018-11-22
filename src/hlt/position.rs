use hlt::direction::Direction;
use std::fmt;
use std::ops;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Position { x, y }
    }

    pub fn directional_offset(&self, d: Direction) -> Position {
        let (dx, dy) = match d {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
            Direction::Still => (0, 0),
        };

        Position {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    pub fn get_surrounding_cardinals(&self) -> Vec<Position> {
        vec![
            self.directional_offset(Direction::North),
            self.directional_offset(Direction::South),
            self.directional_offset(Direction::East),
            self.directional_offset(Direction::West),
        ]
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl ops::Add for Position {
    type Output = Position;

    fn add(self, other: Position) -> Position {
        Position {
            x: self.x + other.x,
            y: self.y + other.y
        }
    }
}