use tricksterstable_rs::games::so8::{get_mcts_move, SixOfVIIIGame};

fn main() {
    for _ in 0..10 {
        // is_experiment.shuffle(&mut rng);
        let mut game = SixOfVIIIGame::new();
        //println!("{:?}", &game);
        game.round = 4; // force single hand
        while game.winner.is_none() {
            let iterations = if game.current_player == 0 || game.current_player == 2 {
                10
            } else {
                1000
            };
            let action = {
                let mut game = game.clone();
                game.experiment = false;
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
            println!("winner: {}", winner);
        }
        for (team, score) in game.scores.iter().enumerate() {
            println!("team {}: {} points", team, score);
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
