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
    Returning,
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
    let mut navi = Navi::new(game.map.width, game.map.height);
    // At this point "game" variable is populated with initial map data.
    // This is a good place to do computationally expensive start-up pre-processing.
    // As soon as you call "ready" function below, the 2 second per turn timer will start.
    Game::ready("hungnd1475");

    Log::log(&format!(
        "Successfully created bot! My Player ID is {}.",
        game.my_id.0
    ));

    let mut ship_states: HashMap<ShipId, ShipAction> = HashMap::new();
    let mut occupied_moves: HashMap<Position, (ShipId, bool)> = HashMap::new();
    let mut ships_queue: Vec<ShipId> = Vec::new();
    let mut waiting_ships: HashMap<ShipId, ShipId> = HashMap::new();

    loop {
        game.update_frame();
        navi.update_frame(&game);

        let me = &game.players[game.my_id.0];
        let map = &mut game.map;

        let mut command_queue: Vec<Command> = Vec::new();

        for ship_id in &me.ship_ids {
            let ship = &game.ships[ship_id];
            let max_halite = game.constants.max_halite;
            ship_states
                .entry(*ship_id)
                .and_modify(|state| {
                    if *state == ShipAction::Collecting {
                        if ship.halite >= max_halite * 9 / 10 {
                            *state = ShipAction::Returning;
                        }
                    } else if ship.halite == 0 {
                        *state = ShipAction::Collecting;
                    }
                }).or_insert(ShipAction::Collecting);
            occupied_moves.insert(ship.position, (*ship_id, false));
        }

        ships_queue.extend(&me.ship_ids);
        while let Some(ship_id) = ships_queue.pop() {
            let ship = &game.ships[&ship_id];
            let state = ship_states[&ship_id];
            Log::log(&format!(
                "Moving ship {} at {} for {:?}",
                ship_id.0, ship.position, state
            ));
            let result = match state {
                ShipAction::Collecting => {
                    get_best_move(ship, map, &navi, &occupied_moves, &waiting_ships).unwrap_or(
                        get_random_move(
                            ship,
                            &mut rng,
                            &navi,
                            &me.shipyard.position,
                            &occupied_moves,
                            &waiting_ships,
                        ),
                    )
                }
                ShipAction::Returning => get_return_move(
                    ship,
                    &navi,
                    &me.shipyard.position,
                    &occupied_moves,
                    &waiting_ships,
                ),
            };
            match result {
                MoveResult::Waiting(blocking_ship) => {
                    Log::log(&format!("Waiting for ship {} to resolve", blocking_ship.0));
                    waiting_ships.insert(blocking_ship, ship_id);
                }
                MoveResult::Resolved(direction) => {
                    let position = navi.normalized_offset(&ship.position, direction);
                    Log::log(&format!("Resolved at {:?} -> {}", direction, position));

                    let &(occupied_ship, _) = occupied_moves.get(&ship.position).unwrap();
                    if occupied_ship == ship_id {
                        occupied_moves.remove(&ship.position);
                    }
                    occupied_moves.insert(position, (ship_id, true));
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
        Log::log(&format!("{} ships waiting", waiting_ships.len()));
        waiting_ships.drain();
        Game::end_turn(&command_queue);
    }
}

enum MoveResult {
    Resolved(Direction),
    Waiting(ShipId),
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
                    MoveResult::Waiting(blocking_ship)
                } else {
                    MoveResult::Resolved(direction)
                }
            } else {
                MoveResult::Resolved(direction)
            }
        }
    }
}

fn get_random_move(
    ship: &Ship,
    rng: &mut XorShiftRng,
    navi: &Navi,
    shipyard_position: &Position,
    occupied_moves: &HashMap<Position, (ShipId, bool)>,
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
    occupied_moves: &HashMap<Position, (ShipId, bool)>,
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
    occupied_moves: &HashMap<Position, (ShipId, bool)>,
    waiting_ships: &HashMap<ShipId, ShipId>,
) -> MoveResult {
    let safe_moves = get_safe_moves(
        &navi.get_unsafe_moves(&ship.position, shipyard_position),
        ship,
        navi,
        occupied_moves,
        waiting_ships,
    );
    let mut result: Option<MoveResult> = None;
    for (direction, blocking_ship) in safe_moves {
        let r = MoveResult::determine(ship, direction, &blocking_ship, waiting_ships);
        match r {
            MoveResult::Resolved(_) => return r,
            MoveResult::Waiting(_) => {
                if result.is_none() {
                    result = Some(r)
                }
            }
        }
    }
    result.unwrap_or(MoveResult::Resolved(Direction::Still))
}

fn get_safe_moves(
    directions: &[Direction],
    ship: &Ship,
    navi: &Navi,
    occupied_moves: &HashMap<Position, (ShipId, bool)>,
    waiting_ships: &HashMap<ShipId, ShipId>,
) -> Vec<(Direction, Option<ShipId>)> {
    directions
        .iter()
        .filter(|&&direction| {
            let position = navi.normalized_offset(&ship.position, direction);
            occupied_moves
                .get(&position)
                .map(|&(ship_id, resolved)| !resolved && !waiting_ships.contains_key(&ship_id))
                .unwrap_or(true)
        }).map(|&direction| {
            let position = navi.normalized_offset(&ship.position, direction);
            (
                direction,
                occupied_moves.get(&position).map(|&(ship_id, _)| ship_id),
            )
        }).collect()
}
