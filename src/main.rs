#[macro_use]
extern crate lazy_static;
extern crate rand;

// use hlt::command::Command;
//use hlt::direction::Direction;
use hlt::game::Game;
use hlt::game_map::GameMap;
use hlt::log::Log;
use hlt::navi::Navi;
use hlt::position::Position;
use hlt::ship::{MoveDecision, Ship};
// use hlt::ShipId;
//use rand::Rng;
// use rand::SeedableRng;
// use rand::XorShiftRng;
use std::collections::{HashMap, HashSet};
// use std::env;
use std::time::Instant;
// use std::time::SystemTime;
// use std::time::UNIX_EPOCH;

mod hlt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum ShipAction {
    Collecting,
    Dropping,
    Finishing,
}

#[derive(Debug)]
struct RichCell {}

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

    let mut ship_actions = HashMap::new();
    let mut ship_goals = HashMap::new();
    let mut ships_queue = Vec::new();
    let mut finishing = false;
    let mut command_queue = Vec::new();
    let mut rich_cells = HashSet::new();
    let mut dropoffs = Vec::new();

    let (min_halite, max_halite) = find_min_max(&game.map);
    let avg_halite = (max_halite - min_halite) / 2;

    // At this point "game" variable is populated with initial map data.
    // This is a good place to do computationally expensive start-up pre-processing.
    // As soon as you call "ready" function below, the 2 second per turn timer will start.
    Game::ready("hungnd1475");

    Log::log(&format!(
        "Successfully created bot! My Player ID is {}.",
        game.my_id.0
    ));

    loop {
        let start_loop = Instant::now();
        game.update_frame();
        navi.update_frame(&game);

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

        rich_cells.retain(|cell_position| {
            let cell = game.map.at_position(cell_position);
            cell.halite >= avg_halite
        });

        for ship_id in &me.ship_ids {
            let ship = game.ships.get(ship_id).unwrap();
            let last_action = ship_actions.get(ship_id).map(|a| *a);

            ship_actions
                .entry(*ship_id)
                .and_modify(|action| {
                    if finishing && *action != ShipAction::Finishing {
                        let distance = game
                            .map
                            .calculate_distance(&ship.position, &me.shipyard.position);
                        if ship.halite >= distance * 2 {
                            *action = ShipAction::Finishing;
                        }
                    } else {
                        match *action {
                            ShipAction::Collecting => {
                                if ship.is_full_by(90) {
                                    *action = ShipAction::Dropping;
                                }
                            }
                            ShipAction::Dropping => {
                                if ship.halite == 0 {
                                    *action = ShipAction::Collecting;
                                }
                            }
                            _ => {}
                        }
                    }
                }).or_insert(ShipAction::Collecting);

            let goal = match ship_actions.get(ship_id).unwrap() {
                ShipAction::Collecting => {
                    Some(ship.scan_for_goal(3, &game.map, &navi, ship_goals.get(ship_id)))
                }
                ShipAction::Dropping => {
                    if let Some(ShipAction::Collecting) = last_action {
                        Some(ship.scan_for_goal(6, &game.map, &navi, ship_goals.get(ship_id)))
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(goal) = goal {
                let goal_cell = game.map.at_position(&goal);
                ship_goals.insert(*ship_id, goal);
                if goal_cell.halite >= avg_halite {
                    rich_cells.insert(goal);
                }
            }
        }

        ships_queue.extend(&me.ship_ids);
        while let Some(ship_id) = ships_queue.pop() {
            let ship = game.ships.get(&ship_id).unwrap();
            let action = ship_actions.get(&ship_id).unwrap();
            Log::log(&format!(
                "Moving ship {} at {} for {:?}",
                ship_id.0, ship.position, action
            ));
            let decision = match action {
                ShipAction::Collecting => {
                    ship.collect_halite(me, &game, &navi, ship_goals.get(&ship_id).unwrap())
                }
                ShipAction::Dropping | ShipAction::Finishing => {
                    ship.drop_halite(me, &game.map, &navi, &dropoffs, finishing)
                }
            };
            match decision {
                MoveDecision::Waiting(_, blocking_ship) => {
                    Log::log(&format!("Waiting for ship {} to resolve", blocking_ship.0));
                    navi.record_waiting(blocking_ship, ship_id);
                }
                MoveDecision::Final(direction) => {
                    let position = ship.position.directional_offset(direction);
                    Log::log(&format!("Resolved at {:?} -> {}", direction, position));
                    navi.resolve(&ship, &position);
                    if let Some(waiting_ship) = navi.waiting.remove(&ship_id) {
                        ships_queue.push(waiting_ship);
                        Log::log(&format!(
                            "Push waiting ship {} back to resolve",
                            waiting_ship.0
                        ));
                    }
                    command_queue.push(ship.move_ship(direction));
                }
                MoveDecision::Dropoff => {
                    command_queue.push(ship.make_dropoff());
                    dropoffs.push(ship.position);
                }
            }
        }

        for (_, ship_id) in navi.waiting.drain() {
            let ship = &game.ships[&ship_id];
            command_queue.push(ship.stay_still());
        }

        if game.turn_number <= 200
            && me.halite >= game.constants.ship_cost
            && navi.is_safe(&me.shipyard.position)
        {
            command_queue.push(me.shipyard.spawn());
        }

        Game::end_turn(command_queue.drain(..));

        let running_time = start_loop.elapsed();
        Log::log(&format!(
            "Commands issued in {}s",
            running_time.as_secs() as f64 + running_time.subsec_nanos() as f64 * 1e-9
        ));
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

fn find_min_max(map: &GameMap) -> (usize, usize) {
    let mut min = std::usize::MAX;
    let mut max = 0;
    for x in 0..map.width {
        for y in 0..map.height {
            let position = Position {
                x: x as i32,
                y: y as i32,
            };
            let cell = map.at_position(&position);
            if min > cell.halite {
                min = cell.halite;
            }
            if max < cell.halite {
                max = cell.halite;
            }
        }
    }
    (min, max)
}

// fn get_closest_cell<'a>(
//     origin: &Position,
//     others: impl Iterator<Item = &'a Position>,
//     map: &GameMap,
// ) -> Option<&'a Position> {
//     others.fold(None, |closest, current| match closest {
//         None => Some(current),
//         Some(closest) => {
//             let closest_distance = map.calculate_distance(origin, closest);
//             let current_distance = map.calculate_distance(origin, current);
//             if closest_distance > current_distance {
//                 Some(current)
//             } else {
//                 Some(closest)
//             }
//         }
//     })
// }
