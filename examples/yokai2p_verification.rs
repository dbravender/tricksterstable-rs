use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, prelude::*, BufReader};
use tricksterstable_rs::games::yokai2p::{ChangeType, Yokai2pDartFormat, Yokai2pGame};

fn main() {
    let _ = verify_against_dart();
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestCase {
    #[serde(rename(serialize = "move", deserialize = "move"))]
    action: Option<i32>,
    game_state: Yokai2pDartFormat,
}

fn remove_empty_changes(mut game: Yokai2pGame) -> Yokai2pGame {
    game.changes.retain(|x| !x.is_empty());
    game
}

fn verify_against_dart() -> io::Result<()> {
    let mut game: Yokai2pGame = Yokai2pGame::new();

    let file = File::open("data/yokai2p.multiplegame.json")?;
    let reader = BufReader::new(file);
    let mut test_count: i32 = 0;

    for line in reader.lines() {
        test_count += 1;
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
            game = test_case.game_state.to_rust().clone();
            continue;
        }
        if test_case.action.is_none() {
            game = test_case.game_state.to_rust().clone();
            continue;
        } else {
            game.no_changes = false;
            game.apply_move(&test_case.action.unwrap());
            game = remove_empty_changes(game);
            if serde_json::to_string(&game).unwrap()
                != serde_json::to_string(&test_case.game_state.to_rust()).unwrap()
            {
                println!("test_count: {}", &test_count);
                println!("move: {}", &test_case.action.unwrap());
                println!("rust: {}", serde_json::to_string(&game).unwrap());
                println!(
                    "dart: {}",
                    serde_json::to_string(&test_case.game_state.to_rust()).unwrap()
                );
                panic!("mismatch");
            }
        }
    }
    println!("Verified {} game states", test_count);
    Ok(())
}
