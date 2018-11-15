#[macro_use]
extern crate lazy_static;
extern crate rand;

use hlt::command::Command;
use hlt::direction::Direction;
use hlt::game::Game;
use hlt::game_map::GameMap;
use hlt::log::Log;
use hlt::navi::Navi;
use hlt::position::Position;
use hlt::ship::Ship;
use hlt::ShipId;
use rand::Rng;
use rand::SeedableRng;
use rand::XorShiftRng;
use std::collections::HashMap;
use std::env;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

mod hlt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum ShipAction {
    Collecting,
    Dropping,
    Finishing,
}

enum Occupation {
    Me(ShipId, bool),
    Opponent,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let rng_seed: u64 = if args.len() > 1 {
        args[1].parse().unwrap()
    } else {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    };
    let seed_bytes: Vec<u8> = (0..16)
        .map(|x| ((rng_seed >> (x % 8)) & 0xFF) as u8)
        .collect();
    let mut rng: XorShiftRng = SeedableRng::from_seed([
        seed_bytes[0],
        seed_bytes[1],
        seed_bytes[2],
        seed_bytes[3],
        seed_bytes[4],
        seed_bytes[5],
        seed_bytes[6],
        seed_bytes[7],
        seed_bytes[8],
        seed_bytes[9],
        seed_bytes[10],
        seed_bytes[11],
        seed_bytes[12],
        seed_bytes[13],
        seed_bytes[14],
        seed_bytes[15],
    ]);

    let mut game = Game::new();
    let navi = Navi::new(game.map.width, game.map.height);
    // At this point "game" variable is populated with initial map data.
    // This is a good place to do computationally expensive start-up pre-processing.
    // As soon as you call "ready" function below, the 2 second per turn timer will start.
    Game::ready("hungnd1475");

    Log::log(&format!(
        "Successfully created bot! My Player ID is {}.",
        game.my_id.0
    ));

    let mut ship_actions: HashMap<ShipId, ShipAction> = HashMap::new();
    let mut occupied_moves: HashMap<Position, Occupation> = HashMap::new();
    let mut ships_queue: Vec<ShipId> = Vec::new();
    let mut waiting_ships: HashMap<ShipId, ShipId> = HashMap::new();
    let mut finishing = false;
    let mut command_queue: Vec<Command> = Vec::new();
    let shipyards_vicinity = {
        let me = &game.players[game.my_id.0];
        let mut v = me.shipyard.position.get_surrounding_cardinals();
        v.push(me.shipyard.position);
        v
    };

    loop {
        game.update_frame();
        //navi.update_frame(&game);

        let me = &game.players[game.my_id.0];
        let remaining_turns = game.constants.max_turns - game.turn_number;

        if !finishing {
            let max_distance = calculate_max_distance(
                me.ship_ids.iter().map(|id| &game.ships[id]),
                &me.shipyard.position,
                &game.map,
            );
            finishing = max_distance >= remaining_turns;
        }

        for player in &game.players {
            for ship_id in &player.ship_ids {
                let ship = &game.ships[ship_id];
                if player.id == game.my_id {
                    occupied_moves.insert(ship.position, Occupation::Me(ship.id, false));
                } else {
                    if !shipyards_vicinity.contains(&ship.position) {
                        occupied_moves.insert(ship.position, Occupation::Opponent);
                    }
                }
            }
        }

        for ship_id in &me.ship_ids {
            let ship = &game.ships[ship_id];
            let max_halite = game.constants.max_halite;
            ship_actions
                .entry(*ship_id)
                .and_modify(|action| {
                    if finishing {
                        *action = ShipAction::Finishing
                    } else {
                        if *action == ShipAction::Collecting {
                            if ship.halite >= max_halite * 9 / 10 {
                                *action = ShipAction::Dropping;
                            }
                        } else if *action == ShipAction::Dropping && ship.halite == 0 {
                            *action = ShipAction::Collecting;
                        }
                    }
                }).or_insert(ShipAction::Collecting);
        }

        ships_queue.extend(&me.ship_ids);
        while let Some(ship_id) = ships_queue.pop() {
            let ship = &game.ships[&ship_id];
            let action = ship_actions[&ship_id];
            Log::log(&format!(
                "Moving ship {} at {} for {:?}",
                ship_id.0, ship.position, action
            ));
            let result = match action {
                ShipAction::Collecting => {
                    get_best_move(ship, &game.map, &navi, &occupied_moves, &waiting_ships)
                        .unwrap_or(get_random_move(
                            ship,
                            &mut rng,
                            &navi,
                            &me.shipyard.position,
                            &occupied_moves,
                            &waiting_ships,
                        ))
                }
                ShipAction::Dropping | ShipAction::Finishing => get_return_move(
                    ship,
                    &navi,
                    &me.shipyard.position,
                    &occupied_moves,
                    &waiting_ships,
                    finishing,
                ),
            };
            match result {
                MoveResult::Waiting(_, blocking_ship) => {
                    Log::log(&format!("Waiting for ship {} to resolve", blocking_ship.0));
                    waiting_ships.insert(blocking_ship, ship_id);
                }
                MoveResult::Resolved(direction) => {
                    let position = navi.normalized_offset(&ship.position, direction);
                    Log::log(&format!("Resolved at {:?} -> {}", direction, position));

                    if occupied_moves
                        .get(&ship.position)
                        .map(|occupation| match *occupation {
                            Occupation::Me(occupied_ship, _) => occupied_ship == ship_id,
                            Occupation::Opponent => unreachable!(),
                        }).unwrap()
                    {
                        occupied_moves.remove(&ship.position);
                    }
                    occupied_moves.insert(position, Occupation::Me(ship_id, true));
                    if let Some(waiting_ship) = waiting_ships.remove(&ship_id) {
                        ships_queue.push(waiting_ship);
                        Log::log(&format!(
                            "Push waiting ship {} back to resolve",
                            waiting_ship.0
                        ));
                    }
                    command_queue.push(ship.move_ship(direction));
                }
            }
        }

        if game.turn_number <= 200
            && me.halite >= game.constants.ship_cost
            && !occupied_moves.contains_key(&me.shipyard.position)
        {
            command_queue.push(me.shipyard.spawn());
        }

        occupied_moves.drain();
        waiting_ships.drain();
        Game::end_turn(command_queue.drain(..));
    }
}

enum MoveResult {
    Resolved(Direction),
    Waiting(Direction, ShipId),
}

impl MoveResult {
    fn determine(
        ship: &Ship,
        direction: Direction,
        blocking_ship: &Option<ShipId>,
        waiting_ships: &HashMap<ShipId, ShipId>,
    ) -> Self {
        if direction == Direction::Still {
            MoveResult::Resolved(direction)
        } else {
            if let Some(blocking_ship) = *blocking_ship {
                if waiting_ships
                    .get(&ship.id)
                    .map(|&ws| ws != blocking_ship)
                    .unwrap_or(true)
                {
                    MoveResult::Waiting(direction, blocking_ship)
                } else {
                    MoveResult::Resolved(direction)
                }
            } else {
                MoveResult::Resolved(direction)
            }
        }
    }
}

fn calculate_max_distance<'a>(
    ships: impl Iterator<Item = &'a Ship>,
    shipyard_position: &Position,
    map: &GameMap,
) -> usize {
    ships.fold(0, |max_distance, ship| {
        let distance = map.calculate_distance(shipyard_position, &ship.position);
        if max_distance < distance {
            distance
        } else {
            max_distance
        }
    })
}

fn get_random_move(
    ship: &Ship,
    rng: &mut XorShiftRng,
    navi: &Navi,
    shipyard_position: &Position,
    occupied_moves: &HashMap<Position, Occupation>,
    waiting_ships: &HashMap<ShipId, ShipId>,
) -> MoveResult {
    let mut safe_moves = get_safe_moves(
        &Direction::get_all_cardinals(),
        ship,
        navi,
        occupied_moves,
        waiting_ships,
    );
    while !safe_moves.is_empty() {
        let index = rng.gen_range(0, safe_moves.len());
        let (direction, blocking_ship) = safe_moves[index];
        let position = navi.normalized_offset(&ship.position, direction);
        if position != *shipyard_position {
            return MoveResult::determine(ship, direction, &blocking_ship, waiting_ships);
        } else {
            safe_moves.remove(index);
        }
    }
    MoveResult::Resolved(Direction::Still)
}

fn get_best_move(
    ship: &Ship,
    map: &GameMap,
    navi: &Navi,
    occupied_moves: &HashMap<Position, Occupation>,
    waiting_ships: &HashMap<ShipId, ShipId>,
) -> Option<MoveResult> {
    let ship_cell = map.at_entity(ship);
    if ship.halite < ship_cell.halite * 10 / 100 {
        return Some(MoveResult::Resolved(Direction::Still));
    }

    let safe_moves = get_safe_moves(
        &Direction::get_all(),
        ship,
        navi,
        occupied_moves,
        waiting_ships,
    );
    let mut best_direction = Direction::Still;
    let mut best_halite = 0;
    let mut blocking_ship: Option<ShipId> = None;
    let mut all_zeroes = true;
    for (direction, bs) in safe_moves {
        let position = navi.normalized_offset(&ship.position, direction);
        let halite = {
            let halite = map.at_position(&position).halite;
            if direction == Direction::Still {
                halite * 3
            } else {
                halite
            }
        };
        all_zeroes = all_zeroes && halite == 0;
        if best_halite <= halite {
            best_halite = halite;
            best_direction = direction;
            blocking_ship = bs;
        }
    }
    if all_zeroes {
        None
    } else {
        Some(MoveResult::determine(
            ship,
            best_direction,
            &blocking_ship,
            waiting_ships,
        ))
    }
}

fn get_return_move(
    ship: &Ship,
    navi: &Navi,
    shipyard_position: &Position,
    occupied_moves: &HashMap<Position, Occupation>,
    waiting_ships: &HashMap<ShipId, ShipId>,
    finishing: bool,
) -> MoveResult {
    let unsafe_moves = navi.get_unsafe_moves(&ship.position, shipyard_position);
    if unsafe_moves.len() == 0 {
        MoveResult::Resolved(Direction::Still)
    } else if finishing
        && navi.normalized_offset(&ship.position, unsafe_moves[0]) == *shipyard_position
    {
        MoveResult::Resolved(unsafe_moves[0])
    } else {
        let safe_moves = get_safe_moves(&unsafe_moves, ship, navi, occupied_moves, waiting_ships);
        let mut result: Option<MoveResult> = None;
        for (direction, blocking_ship) in safe_moves {
            let r = MoveResult::determine(ship, direction, &blocking_ship, waiting_ships);
            match r {
                MoveResult::Resolved(_) => return r,
                MoveResult::Waiting(_, _) => {
                    if result.is_none() {
                        result = Some(r)
                    }
                }
            }
        }
        result.unwrap_or(MoveResult::Resolved(Direction::Still))
    }
}

fn get_safe_moves(
    directions: &[Direction],
    ship: &Ship,
    navi: &Navi,
    occupied_moves: &HashMap<Position, Occupation>,
    waiting_ships: &HashMap<ShipId, ShipId>,
) -> Vec<(Direction, Option<ShipId>)> {
    directions
        .iter()
        .filter(|&&direction| {
            let position = navi.normalized_offset(&ship.position, direction);
            occupied_moves
                .get(&position)
                .map(|occupation| match *occupation {
                    Occupation::Me(ship_id, resolved) => {
                        !resolved && !waiting_ships.contains_key(&ship_id)
                    }
                    Occupation::Opponent => false,
                }).unwrap_or(true)
        }).map(|&direction| {
            let position = navi.normalized_offset(&ship.position, direction);
            (
                direction,
                occupied_moves
                    .get(&position)
                    .and_then(|occupation| match *occupation {
                        Occupation::Me(ship_id, _) => Some(ship_id),
                        Occupation::Opponent => unreachable!(),
                    }),
            )
        }).collect()
}
