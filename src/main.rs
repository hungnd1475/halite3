#[macro_use]
extern crate lazy_static;
extern crate rand;

use hlt::command::Command;
//use hlt::direction::Direction;
use hlt::game::Game;
use hlt::game_map::GameMap;
use hlt::log::Log;
use hlt::navi::{Navi, Occupation};
use hlt::position::Position;
use hlt::ship::{MoveDecision, Ship};
use hlt::ShipId;
//use rand::Rng;
use rand::SeedableRng;
use rand::XorShiftRng;
use std::collections::{HashMap, HashSet};
use std::env;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

mod hlt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum ShipAction {
    Collecting,
    Dropping,
    Finishing,
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
    let mut ship_actions: HashMap<ShipId, ShipAction> = HashMap::new();
    //let mut occupied_moves: HashMap<Position, Occupation> = HashMap::new();
    let mut ships_queue: Vec<ShipId> = Vec::new();
    //let mut waiting_ships: HashMap<ShipId, ShipId> = HashMap::new();
    let mut finishing = false;
    let mut command_queue: Vec<Command> = Vec::new();
    // let shipyard_vicinity = {
    //     let me = &game.players[game.my_id.0];
    //     let mut v = me.shipyard.position.get_surrounding_cardinals();
    //     v.push(me.shipyard.position);
    //     v
    // };
    // let mut rich_cells = HashSet::new();
    let mut ship_goals = HashMap::new();

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

        // rich_cells.retain(|cell_position| {
        //     let cell = game.map.at_position(cell_position);
        //     cell.halite >= 150
        // });

        for ship_id in &me.ship_ids {
            let ship = game.ships.get(ship_id).unwrap();
            let max_halite = game.constants.max_halite;
            let goal = ship.scan_for_goal(3, &game.map, &navi, ship_goals.get(ship_id));

            ship_goals.insert(*ship_id, goal);
            ship_actions
                .entry(*ship_id)
                .and_modify(|action| {
                    if finishing {
                        let distance = game
                            .map
                            .calculate_distance(&ship.position, &me.shipyard.position);
                        if ship.halite >= distance * 2 {
                            *action = ShipAction::Finishing;
                        }
                    } else {
                        match *action {
                            ShipAction::Collecting => {
                                if ship.halite >= max_halite * 9 / 10 {
                                    *action = ShipAction::Dropping;
                                    // rich_cells.extend(
                                    //     ship.vicinity
                                    //         .richest_cells
                                    //         .iter()
                                    //         .filter(|cell| cell.halite >= 150)
                                    //         .map(|cell| cell.position),
                                    // );
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
        }

        ships_queue.extend(&me.ship_ids);
        while let Some(ship_id) = ships_queue.pop() {
            let ship = game.ships.get_mut(&ship_id).unwrap();
            let action = ship_actions.get_mut(&ship_id).unwrap();
            Log::log(&format!(
                "Moving ship {} at {} for {:?}",
                ship_id.0, ship.position, action
            ));
            let decision = match action {
                ShipAction::Collecting => {
                    ship.collect_halite(me, &game.map, &navi, ship_goals.get(&ship_id).unwrap())
                }
                ShipAction::Dropping | ShipAction::Finishing => {
                    ship.drop_halite(me, &game.map, &navi, finishing)
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

                    if navi
                        .occupation_at(&ship.position)
                        .map(|occupation| match *occupation {
                            Occupation::Me(occupied_ship, _) => occupied_ship == ship_id,
                            Occupation::Enemy(_) => false,
                        }).unwrap_or(false)
                    {
                        navi.mark_safe(&ship.position);
                    }
                    navi.resolve(&position, ship_id);
                    if let Some(waiting_ship) = navi.waiting.remove(&ship_id) {
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

// enum MoveResult {
//     Resolved(Direction),
//     Waiting(Direction, ShipId),
// }

// impl MoveResult {
//     fn determine(
//         ship: &Ship,
//         direction: Direction,
//         blocking_ship: &Option<ShipId>,
//         waiting_ships: &HashMap<ShipId, ShipId>,
//     ) -> Self {
//         if direction == Direction::Still {
//             MoveResult::Resolved(direction)
//         } else {
//             if let Some(blocking_ship) = *blocking_ship {
//                 if waiting_ships
//                     .get(&ship.id)
//                     .map(|&ws| ws != blocking_ship)
//                     .unwrap_or(true)
//                 {
//                     MoveResult::Waiting(direction, blocking_ship)
//                 } else {
//                     MoveResult::Resolved(direction)
//                 }
//             } else {
//                 MoveResult::Resolved(direction)
//             }
//         }
//     }
// }

// fn get_random_move(
//     ship: &Ship,
//     rng: &mut XorShiftRng,
//     navi: &Navi,
//     shipyard_position: &Position,
//     occupied_moves: &HashMap<Position, Occupation>,
//     waiting_ships: &HashMap<ShipId, ShipId>,
//     goal_position: &mut Option<Position>,
// ) -> MoveResult {
//     let mut safe_moves = vec![];
//     if let Some(gp) = *goal_position {
//         let mut unsafe_moves = navi.get_unsafe_moves(&ship.position, &gp);
//         if gp == ship.position {
//             for d in unsafe_moves.iter_mut() {
//                 *d = d.invert_direction();
//             }
//             *goal_position = None;
//         }
//         safe_moves = get_safe_moves(&unsafe_moves, ship, navi, false);
//     }
//     if safe_moves.is_empty() {
//         let unsafe_moves = Direction::get_all_cardinals();
//         safe_moves = get_safe_moves(&unsafe_moves, ship, navi, false);
//     }
//     while !safe_moves.is_empty() {
//         let index = rng.gen_range(0, safe_moves.len());
//         let (direction, blocking_ship) = safe_moves[index];
//         let position = navi.normalized_offset(&ship.position, direction);
//         if position != *shipyard_position {
//             return MoveResult::determine(ship, direction, &blocking_ship, waiting_ships);
//         } else {
//             safe_moves.remove(index);
//         }
//     }
//     MoveResult::Resolved(Direction::Still)
// }

// fn get_best_move(ship: &Ship, map: &GameMap, navi: &Navi) -> Option<MoveResult> {
//     let ship_cell = map.at_entity(ship);
//     if (ship.halite as f64) < ((ship_cell.halite as f64) * 0.1).round() {
//         return Some(MoveResult::Resolved(Direction::Still));
//     }

//     let safe_moves = get_safe_moves(&Direction::get_all(), ship, navi, false);
//     let mut best_direction = Direction::Still;
//     let mut best_halite = 0;
//     let mut blocking_ship: Option<ShipId> = None;
//     let mut all_bad = true;
//     for (direction, bs) in safe_moves {
//         let position = ship.position.directional_offset(direction);
//         let halite = {
//             let halite = map.at_position(&position).halite;
//             if direction == Direction::Still {
//                 halite * 3
//             } else {
//                 halite
//             }
//         };
//         all_bad = all_bad && halite <= 15;
//         if best_halite <= halite {
//             best_halite = halite;
//             best_direction = direction;
//             blocking_ship = bs;
//         }
//     }
//     if all_bad {
//         None
//     } else {
//         Some(MoveResult::determine(
//             ship,
//             best_direction,
//             &blocking_ship,
//             waiting_ships,
//         ))
//     }
// }

// fn get_return_move(
//     ship: &Ship,
//     navi: &Navi,
//     shipyard_position: &Position,
//     finishing: bool,
// ) -> MoveResult {
//     let unsafe_moves = navi.get_unsafe_moves(&ship.position, shipyard_position);
//     if unsafe_moves.len() == 0 {
//         MoveResult::Resolved(Direction::Still)
//     } else if finishing
//         && navi.normalize(&ship.position.directional_offset(unsafe_moves[0])) == *shipyard_position
//     {
//         MoveResult::Resolved(unsafe_moves[0])
//     } else {
//         let safe_moves = get_safe_moves(&unsafe_moves, ship, navi, true);
//         let mut result: Option<MoveResult> = None;
//         for (direction, blocking_ship) in safe_moves {
//             let r = MoveResult::determine(ship, direction, &blocking_ship, waiting_ships);
//             match r {
//                 MoveResult::Resolved(_) => return r,
//                 MoveResult::Waiting(_, _) => {
//                     if result.is_none() {
//                         result = Some(r)
//                     }
//                 }
//             }
//         }
//         result.unwrap_or(MoveResult::Resolved(Direction::Still))
//     }
// }

// fn get_safe_moves(
//     directions: &[Direction],
//     ship: &Ship,
//     navi: &Navi,
//     ignore_opponent: bool,
// ) -> Vec<(Direction, Option<ShipId>)> {
//     directions
//         .iter()
//         .filter(|&&direction| {
//             let position = ship.position.directional_offset(direction);
//             navi.occupation_at(&position)
//                 .map(|occupation| match occupation {
//                     Occupation::Me(ship_id, resolved) => {
//                         !resolved && !navi.has_ship_waiting_for(&ship_id)
//                     }
//                     Occupation::Enemy(_) => ignore_opponent,
//                 }).unwrap_or(true)
//         }).map(|&direction| {
//             let position = ship.position.directional_offset(direction);
//             (
//                 direction,
//                 navi.occupation_at(&position)
//                     .and_then(|occupation| match occupation {
//                         Occupation::Me(ship_id, _) => Some(ship_id),
//                         Occupation::Enemy(_) => None,
//                     }),
//             )
//         }).collect()
// }
