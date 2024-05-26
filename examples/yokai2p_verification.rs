use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, prelude::*, BufReader};
use tricksterstable_rs::games::yokai2p::{Card, Change, ChangeType, State, Suit, Yokai2pGame};

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Yokai2pDartFormat {
    state: State,
    trump_card: Option<Card>,
    hands: [Vec<Card>; 2],
    changes: Vec<Vec<Change>>,
    current_trick: HashMap<i32, Card>,
    tricks_taken: HashMap<i32, i32>,
    lead_suit: Option<Suit>,
    scores: HashMap<i32, i32>,
    captured_sevens: [Vec<Card>; 2],
    straw_bottom: [Vec<Option<Card>>; 2],
    straw_top: [Vec<Option<Card>>; 2],
    current_player: usize,
    winner: Option<usize>,
    overall_winner: Option<usize>,
    lead_player: usize,
    round: i32,
}

impl Yokai2pDartFormat {
    fn to_rust(&self) -> Yokai2pGame {
        let trick1: Option<Card> = self.current_trick.get(&0).cloned();
        let trick2: Option<Card> = self.current_trick.get(&1).cloned();
        let mut changes = self.changes.clone();
        for (index, _change) in self.changes.iter().enumerate() {
            // There is a bug where duplicate HidePlayable
            // entries are added on the Dart side
            // I wasn't able to figure out the bug to add
            // it to the Rust side so I'm removing the duplicates
            // here
            let mut seen = HashSet::new();
            changes[index].retain(|x| seen.insert(serde_json::to_string(x).unwrap()));
        }
        changes.retain(|x| !x.is_empty());
        Yokai2pGame {
            state: self.state.clone(),
            trump_card: self.trump_card.clone(),
            hands: self.hands.clone(),
            changes,
            current_trick: [trick1, trick2],
            tricks_taken: [
                *self.tricks_taken.get(&0).unwrap_or(&0),
                *self.tricks_taken.get(&1).unwrap_or(&0),
            ],
            lead_suit: self.lead_suit.clone(),
            scores: [
                *self.scores.get(&0).unwrap_or(&0),
                *self.scores.get(&1).unwrap_or(&0),
            ],
            hand_scores: [0, 0],
            voids: [HashSet::new(), HashSet::new()],
            captured_sevens: self.captured_sevens.clone(),
            straw_bottom: self.straw_bottom.clone(),
            straw_top: self.straw_top.clone(),
            current_player: self.current_player,
            winner: self.winner,
            overall_winner: self.overall_winner,
            lead_player: self.lead_player,
            round: self.round,
            no_changes: false,
        }
    }
}

fn verify_against_dart() -> io::Result<()> {
    let mut game: Yokai2pGame = Yokai2pGame::new();

    let file = File::open("data/yokai2p.singlegame.json")?;
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
            println!("shuffled");
            game = test_case.game_state.to_rust().clone();
            continue;
        }
        if test_case.action.is_none() {
            println!("no action");
            game = test_case.game_state.to_rust().clone();
            continue;
        } else {
            //println!("rust: {}", serde_json::to_string(&game).unwrap());
            //println!("move: {}", &test_case.action.unwrap());
            game.no_changes = false;
            game.apply_move(&test_case.action.unwrap());
            //println!("rust: {}", serde_json::to_string(&game).unwrap());
            if game != test_case.game_state.to_rust() {
                //println!("test_count: {}", &test_count);
                //println!("move: {}", &test_case.action.unwrap());
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
