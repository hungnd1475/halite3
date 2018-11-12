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
use std::collections::{HashMap, HashSet};
use std::env;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

mod hlt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum ShipState {
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

    let mut ship_states: HashMap<ShipId, ShipState> = HashMap::new();
    let mut occupied_positions: HashSet<Position> = HashSet::with_capacity(400);

    loop {
        game.update_frame();
        navi.update_frame(&game);

        let me = &game.players[game.my_id.0];
        let map = &mut game.map;

        let mut command_queue: Vec<Command> = Vec::new();

        if game.turn_number == game.constants.max_turns {}

        for ship_id in &me.ship_ids {
            let ship = &game.ships[ship_id];
            let max_halite = game.constants.max_halite;
            ship_states
                .entry(*ship_id)
                .and_modify(|state| {
                    if *state == ShipState::Collecting {
                        if ship.halite >= max_halite * 9 / 10 {
                            *state = ShipState::Returning;
                        }
                    } else if ship.halite == 0 {
                        *state = ShipState::Collecting;
                    }
                }).or_insert(ShipState::Collecting);
            occupied_positions.insert(ship.position);
        }

        for ship_id in &me.ship_ids {
            let ship = &game.ships[ship_id];
            let state = ship_states.get(ship_id).unwrap();
            match *state {
                ShipState::Collecting => {
                    let direction = get_best_move(ship, map, &occupied_positions, &navi).unwrap_or(
                        get_random_move(
                            ship,
                            &occupied_positions,
                            &mut rng,
                            &navi,
                            &me.shipyard.position,
                        ),
                    );
                    if direction != Direction::Still {
                        let position = ship.position.directional_offset(direction);
                        let position = navi.normalize(&position);
                        occupied_positions.insert(position);
                        occupied_positions.remove(&ship.position);
                    }
                    command_queue.push(ship.move_ship(direction));
                }
                ShipState::Returning => {
                    let direction = navi.naive_navigate(ship, &me.shipyard.position);
                    let position = ship.position.directional_offset(direction);
                    if occupied_positions.contains(&position) {
                        command_queue.push(ship.stay_still());
                    } else {
                        command_queue.push(ship.move_ship(direction));
                        occupied_positions.insert(position);
                        occupied_positions.remove(&ship.position);
                    }
                }
            }
        }

        if game.turn_number <= 200
            && me.halite >= game.constants.ship_cost
            && navi.is_safe(&me.shipyard.position)
        {
            command_queue.push(me.shipyard.spawn());
        }

        occupied_positions.drain();
        Game::end_turn(&command_queue);
    }
}

fn get_random_move(
    ship: &Ship,
    occupied_positions: &HashSet<Position>,
    rng: &mut XorShiftRng,
    navi: &Navi,
    shipyard_position: &Position,
) -> Direction {
    let mut possible_directions = Direction::get_all_cardinals();
    while possible_directions.len() > 0 {
        let index = rng.gen_range(0, possible_directions.len());
        let direction = possible_directions[index];
        let position = ship.position.directional_offset(direction);
        let position = navi.normalize(&position);
        if !occupied_positions.contains(&position) && position != *shipyard_position {
            return direction;
        } else {
            possible_directions.remove(index);
        }
    }
    Direction::Still
}

fn get_best_move(
    ship: &Ship,
    map: &GameMap,
    occupied_positions: &HashSet<Position>,
    navi: &Navi,
) -> Option<Direction> {
    let cell = map.at_entity(ship);
    let mut best_direction = Direction::Still;
    let mut best_halite = cell.halite * 3;
    let mut all_zeroes = best_halite == 0;
    for next_dir in Direction::get_all_cardinals() {
        let next_position = ship.position.directional_offset(next_dir);
        let next_position = navi.normalize(&next_position);
        let next_cell = map.at_position(&next_position);
        all_zeroes = all_zeroes && next_cell.halite == 0;
        if !occupied_positions.contains(&next_position) && best_halite <= next_cell.halite {
            best_direction = next_dir;
            best_halite = next_cell.halite;
        }
    }
    if all_zeroes {
        None
    } else {
        Some(best_direction)
    }
}
