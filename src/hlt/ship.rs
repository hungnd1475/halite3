use hlt::command::Command;
use hlt::direction::Direction;
use hlt::entity::Entity;
use hlt::game_map::GameMap;
use hlt::input::Input;
use hlt::log::Log;
use hlt::map_cell::{MapCell, Structure};
use hlt::navi::{Navi, Occupation};
use hlt::player::Player;
use hlt::position::Position;
use hlt::PlayerId;
use hlt::ShipId;

pub struct Ship {
    pub owner: PlayerId,
    pub id: ShipId,
    pub position: Position,
    pub halite: usize,
    max_halite: usize,
}

impl Ship {
    pub fn is_full(&self) -> bool {
        self.halite >= self.max_halite
    }

    pub fn make_dropoff(&self) -> Command {
        Command::transform_ship_into_dropoff_site(self.id)
    }

    pub fn move_ship(&self, direction: Direction) -> Command {
        Command::move_ship(self.id, direction)
    }

    pub fn stay_still(&self) -> Command {
        Command::move_ship(self.id, Direction::Still)
    }

    pub fn generate(input: &mut Input, player_id: PlayerId, max_halite: usize) -> Ship {
        input.read_and_parse_line();
        let id = ShipId(input.next_usize());
        let x = input.next_i32();
        let y = input.next_i32();
        let halite = input.next_usize();

        Ship {
            owner: player_id,
            id,
            position: Position { x, y },
            halite,
            max_halite,
        }
    }

    pub fn scan_for_goal(
        &self,
        radius: i32,
        map: &GameMap,
        navi: &Navi,
        goal: Option<&Position>,
    ) -> Position {
        let (mut goal, mut goal_halite) = goal
            .map(|goal| {
                let cell = map.at_position(&goal);
                (*goal, cell.halite)
            }).unwrap_or((Position { x: 0, y: 0 }, 0));

        for x in (self.position.x - radius)..=(self.position.x + radius) {
            for y in (self.position.y - radius)..=(self.position.y + radius) {
                let position = Position { x, y };
                let halite = map.at_position(&position).halite;

                if goal_halite < halite {
                    goal_halite = halite;
                    goal = position;
                }
            }
        }
        goal
    }

    pub fn drop_halite(
        &self,
        player: &Player,
        map: &GameMap,
        navi: &Navi,
        finishing: bool,
    ) -> MoveDecision {
        let unsafe_moves = navi.get_unsafe_moves(&self.position, &player.shipyard.position);
        if unsafe_moves.len() == 0 {
            MoveDecision::Final(Direction::Still)
        } else if finishing
            && self.position.directional_offset(unsafe_moves[0]) == player.shipyard.position
        {
            MoveDecision::Final(unsafe_moves[0])
        } else {
            let mut enemy_blocked = None;
            let mut safe_moves = self.get_safe_moves(&unsafe_moves, navi);
            safe_moves.sort_by(|a, b| a.threat_level.cmp(&b.threat_level));

            for mv in safe_moves {
                let decision = MoveDecision::decide(self, &mv, navi, false);
                if let Some(decision) = decision {
                    return decision;
                } else {
                    enemy_blocked = Some(mv);
                }
            }

            if let Some(enemy_blocked) = enemy_blocked {
                let unsafe_moves = enemy_blocked.direction.get_orthorgonal();
                let mut safe_moves = self.get_safe_moves(&unsafe_moves, navi);
                safe_moves.sort_by(|a, b| a.threat_level.cmp(&b.threat_level));

                for mv in safe_moves {
                    let decision = MoveDecision::decide(self, &mv, navi, false);
                    if let Some(decision) = decision {
                        return decision;
                    }
                }
            }

            MoveDecision::Final(Direction::Still)
        }
    }

    pub fn collect_halite(
        &self,
        player: &Player,
        map: &GameMap,
        navi: &Navi,
        goal: &Position,
    ) -> MoveDecision {
        let ship_cell = map.at_entity(self);
        if (self.halite as f64) < ((ship_cell.halite as f64) * 0.1).round() {
            return MoveDecision::Final(Direction::Still);
        }

        let goal_moves = {
            let mut moves = navi.get_unsafe_moves(&self.position, goal);
            if self.position != player.shipyard.position {
                moves.push(Direction::Still);
            }
            moves
        };
        Log::log(&format!("Goal moves: {:?}", goal_moves));

        self.get_best_move(&goal_moves, map, navi)
            .map(|mv| MoveDecision::decide(self, &mv, navi, false).unwrap())
            .unwrap_or_else(|| {
                let other_moves: Vec<Direction> = Direction::get_all()
                    .into_iter()
                    .filter(|d| !goal_moves.contains(d))
                    .collect();
                Log::log(&format!("Other moves: {:?}", other_moves));

                self.get_best_move(&other_moves, map, navi)
                    .map(|mv| MoveDecision::decide(self, &mv, navi, false).unwrap())
                    .unwrap_or(MoveDecision::Final(Direction::Still))
            })
    }

    fn get_best_move(
        &self,
        directions: &[Direction],
        map: &GameMap,
        navi: &Navi,
    ) -> Option<SafeMove> {
        let safe_moves = self.get_safe_moves(&directions, navi);
        let mut best_move = None;
        let mut best_halite = 0;

        for mv in safe_moves {
            let position = self.position.directional_offset(mv.direction);
            let mut halite = map.at_position(&position).halite;
            if halite >= 20 && mv.direction == Direction::Still {
                halite *= 3;
            } else if halite < 20 && mv.direction == Direction::Still {
                continue;
            }

            if mv
                .blocking_ship
                .map(|occupation| match occupation {
                    Occupation::Enemy(_) => false,
                    Occupation::Me(_, _) => true,
                }).unwrap_or(true)
            {
                if best_move.is_none() {
                    best_halite = halite;
                    best_move = Some(mv);
                } else {
                    if best_halite < halite {
                        best_halite = halite;
                        best_move = Some(mv);
                    } else if best_halite == halite {
                        if let Some(best_move) = &mut best_move {
                            if best_move.direction == Direction::Still {
                                best_halite = halite;
                                *best_move = mv;
                            }
                        }
                    }
                }
            }
        }
        best_move
    }

    fn get_safe_moves(&self, directions: &[Direction], navi: &Navi) -> Vec<SafeMove> {
        fn check_for_threats(direction: Direction, position: &Position, navi: &Navi) -> i32 {
            let mut lookup_directions = direction.get_orthorgonal();
            lookup_directions.push(direction);
            let lookup_positions = lookup_directions
                .iter()
                .map(|&d| position.directional_offset(d));
            let mut threats = 0;
            for p in lookup_positions {
                if navi
                    .occupation_at(&p)
                    .map(|occupation| match occupation {
                        Occupation::Me(_, _) => false,
                        Occupation::Enemy(_) => true,
                    }).unwrap_or(false)
                {
                    threats += 1;
                }
            }
            threats
        }

        directions
            .iter()
            .filter(|&&direction| {
                let position = self.position.directional_offset(direction);
                navi.occupation_at(&position)
                    .map(|occupation| match *occupation {
                        Occupation::Me(ship_id, is_final) => {
                            !is_final && !navi.has_ship_waiting_for(&ship_id)
                        }
                        Occupation::Enemy(_) => true,
                    }).unwrap_or(true)
            }).map(|&direction| {
                let position = self.position.directional_offset(direction);
                let blocking_ship = navi.occupation_at(&position).map(|occupation| *occupation);
                let threat_level = check_for_threats(direction, &position, navi);
                SafeMove {
                    direction,
                    blocking_ship,
                    threat_level,
                }
            }).collect()
    }
}

// pub struct Vicinity {
//     pub richest_cells: Vec<MapCell>,
//     pub goal: Position,
//     pub goal_moves: Vec<Direction>,
//     pub other_moves: Vec<Direction>,
//     //enemies: Vec<Position>,
// }

#[derive(Debug, Copy, Clone)]
struct SafeMove {
    direction: Direction,
    blocking_ship: Option<Occupation>,
    threat_level: i32,
}

pub enum MoveDecision {
    Waiting(Direction, ShipId),
    Final(Direction),
}

impl MoveDecision {
    fn decide(ship: &Ship, mv: &SafeMove, navi: &Navi, offensive: bool) -> Option<Self> {
        if mv.direction == Direction::Still {
            Some(MoveDecision::Final(mv.direction))
        } else {
            if let Some(blocking_ship) = mv.blocking_ship {
                match blocking_ship {
                    Occupation::Me(bs, _) => {
                        if navi
                            .waiting
                            .get(&ship.id)
                            .map(|&ws| ws != bs)
                            .unwrap_or(true)
                        {
                            Some(MoveDecision::Waiting(mv.direction, bs))
                        } else {
                            Some(MoveDecision::Final(mv.direction))
                        }
                    }
                    Occupation::Enemy(_) => if offensive {
                        Some(MoveDecision::Final(mv.direction))
                    } else {
                        None
                    },
                }
            } else {
                Some(MoveDecision::Final(mv.direction))
            }
        }
    }
}

impl Entity for Ship {
    fn owner(&self) -> PlayerId {
        self.owner
    }

    fn position(&self) -> Position {
        self.position
    }
}
