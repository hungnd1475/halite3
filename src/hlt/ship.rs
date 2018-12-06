use hlt::command::Command;
use hlt::direction::Direction;
use hlt::entity::Entity;
use hlt::game::Game;
use hlt::game_map::GameMap;
use hlt::input::Input;
use hlt::log::Log;
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

    pub fn is_full_by(&self, percentage: usize) -> bool {
        self.halite >= self.max_halite * percentage / 100
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
        _navi: &Navi,
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
        dropoffs: &[Position],
        finishing: bool,
    ) -> MoveDecision {
        let nearest_dropoff = dropoffs.iter().fold(&player.shipyard.position, |a, b| {
            let ad = map.calculate_distance(a, &self.position);
            let bd = map.calculate_distance(b, &self.position);
            if ad > bd {
                b
            } else {
                a
            }
        });

        let distance = map.calculate_distance(nearest_dropoff, &self.position);
        if distance > map.width / 3 && player.halite >= 4000 - self.halite - map.at_entity(self).halite {
            return MoveDecision::Dropoff;
        }

        let unsafe_moves = navi.get_unsafe_moves(&self.position, nearest_dropoff);
        if unsafe_moves.len() == 0 {
            MoveDecision::Final(Direction::Still)
        } else if finishing && self.position.directional_offset(unsafe_moves[0]) == *nearest_dropoff
        {
            MoveDecision::Final(unsafe_moves[0])
        } else {
            let mut enemy_blocked = None;
            let mut safe_moves = self.get_safe_moves(&unsafe_moves, navi);
            safe_moves.sort_by(|a, b| a.threat_level.cmp(&b.threat_level));

            for mv in safe_moves {
                if mv
                    .blocking_ship
                    .map(|occupation| match occupation {
                        Occupation::Enemy(_) => true,
                        Occupation::Me(_, _) => false,
                    }).unwrap_or(false)
                {
                    enemy_blocked = Some(mv)
                } else {
                    return MoveDecision::decide(self, &mv, navi);
                }
            }

            if let Some(enemy_blocked) = enemy_blocked {
                let unsafe_moves = enemy_blocked.direction.get_orthorgonal();
                let mut safe_moves = self.get_safe_moves(&unsafe_moves, navi);
                safe_moves.sort_by(|a, b| a.threat_level.cmp(&b.threat_level));

                for mv in safe_moves {
                    if mv
                        .blocking_ship
                        .map(|occupation| match occupation {
                            Occupation::Me(_, _) => true,
                            Occupation::Enemy(_) => false,
                        }).unwrap_or(true)
                    {
                        return MoveDecision::decide(self, &mv, navi);
                    }
                }
            }

            MoveDecision::Final(Direction::Still)
        }
    }

    pub fn collect_halite(
        &self,
        player: &Player,
        game: &Game,
        navi: &Navi,
        goal: &Position,
    ) -> MoveDecision {
        let ship_cell = game.map.at_entity(self);
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

        self.get_best_move(&goal_moves, game, navi)
            .map(|mv| MoveDecision::decide(self, &mv, navi))
            .unwrap_or_else(|| {
                let other_moves: Vec<Direction> = Direction::get_all()
                    .into_iter()
                    .filter(|d| !goal_moves.contains(d))
                    .collect();
                Log::log(&format!("Other moves: {:?}", other_moves));

                self.get_best_move(&other_moves, game, navi)
                    .map(|mv| MoveDecision::decide(self, &mv, navi))
                    .unwrap_or(MoveDecision::Final(Direction::Still))
            })
    }

    fn get_best_move<'a>(
        &self,
        directions: &[Direction],
        game: &Game,
        navi: &'a Navi,
    ) -> Option<SafeMove<'a>> {
        let safe_moves = self.get_safe_moves(&directions, navi);
        let mut best_move = None;
        let mut best_halite = 0;

        for mv in safe_moves {
            let position = self.position.directional_offset(mv.direction);
            let mut halite = game.map.at_position(&position).halite;
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

    fn get_safe_moves<'a>(&self, directions: &[Direction], navi: &'a Navi) -> Vec<SafeMove<'a>> {
        fn check_for_threats(position: &Position, navi: &Navi) -> i32 {
            let lookup_directions = Direction::get_all_cardinals();
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
                navi.is_safe(&position)
            }).map(|&direction| {
                let position = self.position.directional_offset(direction);
                let blocking_ship = navi.occupation_at(&position);
                let threat_level = check_for_threats(&position, navi);
                SafeMove {
                    direction,
                    blocking_ship,
                    threat_level,
                }
            }).collect()
    }
}

#[derive(Debug)]
struct SafeMove<'a> {
    direction: Direction,
    blocking_ship: Option<&'a Occupation>,
    threat_level: i32,
}

pub enum MoveDecision {
    Waiting(Direction, ShipId),
    Final(Direction),
    Dropoff,
}

impl MoveDecision {
    fn decide(ship: &Ship, mv: &SafeMove, navi: &Navi) -> Self {
        if mv.direction == Direction::Still {
            MoveDecision::Final(mv.direction)
        } else {
            if let Some(blocking_ship) = mv.blocking_ship {
                match blocking_ship {
                    Occupation::Me(bs, _) => {
                        if navi
                            .waiting
                            .get(&ship.id)
                            .map(|&ws| ws != *bs)
                            .unwrap_or(true)
                        {
                            MoveDecision::Waiting(mv.direction, *bs)
                        } else {
                            MoveDecision::Final(mv.direction)
                        }
                    }
                    Occupation::Enemy(_) => MoveDecision::Final(mv.direction),
                }
            } else {
                MoveDecision::Final(mv.direction)
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
