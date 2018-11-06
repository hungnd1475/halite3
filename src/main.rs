#[macro_use]
extern crate lazy_static;
extern crate rand;

use hlt::command::Command;
use hlt::direction::Direction;
use hlt::game::Game;
use hlt::log::Log;
use hlt::navi::Navi;
use hlt::position::Position;
use hlt::ShipId;
// use rand::Rng;
// use rand::SeedableRng;
// use rand::XorShiftRng;
use std::collections::{HashMap, HashSet};
// use std::env;
// use std::time::SystemTime;
// use std::time::UNIX_EPOCH;

mod hlt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum ShipState {
    Collecting,
    Returning,
}

fn main() {
    // let args: Vec<String> = env::args().collect();
    // let rng_seed: u64 = if args.len() > 1 {
    //     args[1].parse().unwrap()
    // } else {
    //     SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_secs()
    // };
    // let seed_bytes: Vec<u8> = (0..16)
    //     .map(|x| ((rng_seed >> (x % 8)) & 0xFF) as u8)
    //     .collect();
    // let mut rng: XorShiftRng = SeedableRng::from_seed([
    //     seed_bytes[0],
    //     seed_bytes[1],
    //     seed_bytes[2],
    //     seed_bytes[3],
    //     seed_bytes[4],
    //     seed_bytes[5],
    //     seed_bytes[6],
    //     seed_bytes[7],
    //     seed_bytes[8],
    //     seed_bytes[9],
    //     seed_bytes[10],
    //     seed_bytes[11],
    //     seed_bytes[12],
    //     seed_bytes[13],
    //     seed_bytes[14],
    //     seed_bytes[15],
    // ]);

    let mut game = Game::new();
    let mut navi = Navi::new(game.map.width, game.map.height);
    // At this point "game" variable is populated with initial map data.
    // This is a good place to do computationally expensive start-up pre-processing.
    // As soon as you call "ready" function below, the 2 second per turn timer will start.
    Game::ready("MyRustBot");

    Log::log(&format!(
        "Successfully created bot! My Player ID is {}.",
        game.my_id.0
    ));

    let mut ship_states: HashMap<ShipId, ShipState> = HashMap::new();
    let mut next_positions: HashSet<Position> = HashSet::with_capacity(400);

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
                        if ship.halite >= max_halite * 5 / 10 {
                            *state = ShipState::Returning;
                        }
                    } else if ship.halite == 0 {
                        *state = ShipState::Collecting;
                    }
                }).or_insert(ShipState::Collecting);
            next_positions.insert(ship.position);
        }

        for ship_id in &me.ship_ids {
            let ship = &game.ships[ship_id];
            let cell = map.at_entity(ship);
            let state = ship_states.get(ship_id).unwrap();
            match *state {
                ShipState::Collecting => {
                    if ship.halite < cell.halite * 10 / 100 {
                        command_queue.push(ship.stay_still());
                    } else {
                        let mut max_dir = Direction::Still;
                        let mut max_pos = ship.position;
                        let mut max_halite = map.at_entity(ship).halite;
                        for next_dir in Direction::get_all_cardinals() {
                            let next_pos = ship.position.directional_offset(next_dir);
                            let next_cell = map.at_position(&next_pos);
                            if !next_positions.contains(&next_pos) && max_halite < next_cell.halite {
                                max_dir = next_dir;
                                max_pos = next_pos;
                                max_halite = next_cell.halite;
                            }
                        }
                        next_positions.insert(max_pos);
                        command_queue.push(ship.move_ship(max_dir));
                    }
                }
                ShipState::Returning => {
                    let direction = navi.naive_navigate(ship, &me.shipyard.position);
                    let position = ship.position.directional_offset(direction);
                    if next_positions.contains(&position) {
                        command_queue.push(ship.stay_still());
                    } else {
                        command_queue.push(ship.move_ship(direction));
                        next_positions.insert(position);
                    }
                }
            }
        }

        if me.ship_ids.len() < 10
            && game.turn_number <= 200
            && me.halite >= game.constants.ship_cost * 13 / 10
            && navi.is_safe(&me.shipyard.position)
        {
            command_queue.push(me.shipyard.spawn());
        }

        next_positions.drain();
        Game::end_turn(&command_queue);
    }
}
