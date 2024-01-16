use games::szs::{ChangeType, Game};
use ismcts::{Game as MctsGame, IsmctsHandler};
use rand::random;
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::Hash;
use std::io::{self, prelude::*, BufReader};
use std::time::Instant;

pub mod games;
pub mod utils;

fn main() {
    //let _ = verify_against_dart();
    //let _ = random_play();
    let _ = ismcts_play();
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestCase {
    #[serde(rename(serialize = "move", deserialize = "move"))]
    action: Option<i32>,
    game_state: games::szs::Game,
}

fn verify_against_dart() -> io::Result<()> {
    let mut game: Game = games::szs::Game::new();

    let file = File::open("data/testrun.json.all")?;
    let reader = BufReader::new(file);
    let mut test_count: i32 = 0;

    for line in reader.lines() {
        test_count = test_count + 1;
        let test_case: TestCase = serde_json::from_str(&line.unwrap()).unwrap();
        if test_case
            .game_state
            .changes
            .iter()
            .filter(|cs| cs.iter().any(|c| c.change_type == ChangeType::Shuffle))
            .count()
            > 0
        {
            // Can't easily test this case since we don't have the intermediate step where
            // the shuffle occurred
            game = test_case.game_state.clone();
            continue;
        }
        if test_case.action.is_none() {
            game = test_case.game_state.clone();
        } else {
            game = game.clone_and_apply_move(test_case.action.unwrap());
            game.dealer = test_case.game_state.dealer.clone();
            game.voids = vec![HashSet::new(), HashSet::new(), HashSet::new()];
            game.draw_decks = test_case.game_state.draw_decks.clone();
            if game != test_case.game_state {
                println!("test_count: {}", &test_count);
                println!("move: {}", &test_case.action.unwrap());
                println!("rust: {}", serde_json::to_string(&game).unwrap());
                println!(
                    "dart: {}",
                    serde_json::to_string(&test_case.game_state).unwrap()
                );
                panic!("mismatch");
            }
        }
    }
    println!("Verified {} game states", test_count);
    Ok(())
}

fn random_play() {
    // let mut scores = vec![0, 0, 0];

    let start = Instant::now();
    for _ in 0..10000 {
        let mut game = games::szs::Game::new();
        while game.winner.is_none() {
            let mut actions = game.get_moves();
            actions.shuffle(&mut thread_rng());
            game = game.clone_and_apply_move(*actions.first().expect("should have a move to make"));
        }
        // print!(".");
        // io::stdout().flush().unwrap();
        // scores = (0..3).map(|x| scores[x] + game.scores[x]).collect();
    }
    // println!("\n{:?}", scores);

    let duration = start.elapsed();

    println!("Time elapsed for 10,000 games in Rust: {:?}", duration);
}

trait MoveMaker {
    fn get_move(&self, game: &Game) -> i32;
    fn get_name(&self) -> &str;
}

struct MCTSMove {}
struct RandomMove {
    id: String,
}

impl MoveMaker for MCTSMove {
    fn get_move(&self, game: &Game) -> i32 {
        let mut new_game = game.clone();
        new_game.round = 4;
        let mut ismcts = IsmctsHandler::new(new_game);
        let parallel_threads: usize = 8;
        ismcts.run_iterations(parallel_threads, 1000 / parallel_threads);
        // ismcts.debug_select();
        ismcts.best_move().expect("should have a move to make")
    }

    fn get_name(&self) -> &str {
        "MCTS"
    }
}

impl MoveMaker for RandomMove {
    fn get_move(&self, game: &Game) -> i32 {
        let mut actions = game.get_moves();
        actions.shuffle(&mut thread_rng());
        *actions.first().expect("should have a move to make")
    }

    fn get_name(&self) -> &str {
        &self.id
    }
}

pub fn ismcts_play() {
    let mut players: Vec<Box<dyn MoveMaker>> = vec![
        Box::new(MCTSMove {}),
        Box::new(RandomMove {
            id: String::from("random1"),
        }),
        Box::new(RandomMove {
            id: String::from("random2"),
        }),
    ];
    let mut wins: HashMap<String, usize> = HashMap::new();
    for _i in 0..33 {
        let mut start_game = games::szs::Game::new();
        start_game.round = 4;
        for cycle in 0..3 {
            let mut total_move_time: HashMap<String, u128> = HashMap::new();
            let mut game = start_game.clone();
            let player = players.pop().unwrap();
            players.insert(0, player);
            let mut total_moves = 0;
            while game.winner.is_none() {
                total_moves += 1;
                let start = Instant::now();
                let mov = players[game.current_player() as usize].get_move(&game);
                let duration = start.elapsed();
                *total_move_time
                    .entry(
                        players[game.current_player() as usize]
                            .get_name()
                            .to_owned(),
                    )
                    .or_insert(0) += duration.as_millis();
                game = game.clone_and_apply_move(mov);
            }

            let high_score = game.scores.iter().reduce(|x, y| if x > y { x } else { y });
            for player in 0..3 {
                if game.scores[player] == *high_score.unwrap() {
                    *wins
                        .entry(players[player].get_name().to_owned())
                        .or_insert(0) += 1;
                }
            }
            println!("total moves: {:?}", total_moves);
            println!("wins: {:?}", wins);
            println!("total_move_time: {:?}", total_move_time);
        }
    }
}
