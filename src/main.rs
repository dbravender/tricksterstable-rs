use games::szs::{ChangeType, Game};
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, prelude::*, BufReader};

pub mod games;

fn main() {
    let _ = verify_against_dart();
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestCase {
    #[serde(rename(serialize = "move", deserialize = "move"))]
    action: Option<i32>,
    game_state: Game,
}

fn verify_against_dart() -> io::Result<()> {
    let mut game: Game = Game::new();

    let file = File::open("data/testrun.json")?;
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
    let mut scores = vec![0, 0, 0];
    for _ in 0..1000 {
        let mut game = games::szs::Game::new();
        while game.winner.is_none() {
            let mut actions = game.get_moves();
            actions.shuffle(&mut thread_rng());
            game = game.clone_and_apply_move(*actions.first().expect("should have a move to make"));
        }
        print!(".");
        io::stdout().flush().unwrap();
        scores = (0..3).map(|x| scores[x] + game.scores[x]).collect();
    }
    println!("\n{:?}", scores);
}
