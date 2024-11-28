use rand::{seq::SliceRandom, thread_rng};
use tricksterstable_rs::games::kansascity::{get_mcts_move, KansasCityGame};

fn main() {
    let mut rng = thread_rng();
    let mut is_experiment = vec![true, true, false, false];
    for _ in 0..1000 {
        is_experiment.shuffle(&mut rng);
        let mut game = KansasCityGame::new();
        //println!("{:?}", &game);
        game.round = 5; // force single hand
        while game.winner.is_none() {
            let iterations = 500;
            let action = {
                let mut game = game.clone();
                game.experiment = is_experiment[game.current_player];
                get_mcts_move(&game, iterations, false)
            };
            game.apply_move(action);
        }
        let max_score = game.scores.iter().max().unwrap();
        let winners: Vec<usize> = game
            .scores
            .iter()
            .enumerate()
            .filter(|(_player, score)| *score == max_score)
            .map(|(player, _score)| player)
            .collect();
        for winner in winners {
            println!("winner: {}", get_name(&is_experiment, winner));
        }
        for (player, score) in game.scores.iter().enumerate() {
            println!("score {}: {}", get_name(&is_experiment, player), score);
        }
    }
}

#[inline]
fn get_name(is_experiment: &Vec<bool>, player: usize) -> String {
    if is_experiment[player] {
        "experiment".to_string()
    } else {
        "baseline".to_string()
    }
}
