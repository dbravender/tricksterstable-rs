use rand::{thread_rng, Rng};
use tricksterstable_rs::games::kansascity::{get_mcts_move, KansasCityGame};

fn main() {
    let mut rnd = thread_rng();
    for _ in 0..1000 {
        let mut game = KansasCityGame::new();
        game.dealer = rnd.gen_range(0..4);
        game.current_player = game.dealer;
        //println!("{:?}", &game);
        game.round = 5; // force single hand
        while game.winner.is_none() {
            let iterations = 500;
            let action = if game.current_player == 0 || game.current_player == 2 {
                let mut game = game.clone();
                game.experiment = false;
                get_mcts_move(&game, iterations, false)
            } else {
                let mut game = game.clone();
                game.experiment = true;
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
        for (player, score) in game.scores.iter().enumerate() {
            println!("score {}: {}", player, score);
        }
    }
}
