use rand::{thread_rng, Rng};
use tricksterstable_rs::games::kansascity::{get_mcts_move, KansasCityGame};

fn main() {
    let mut rnd = thread_rng();
    for _ in 0..1000 {
        let mut game = KansasCityGame::new();
        game.dealer = if rnd.gen_range(0..100) > 50 { 0 } else { 1 };
        game.current_player = game.dealer;
        //println!("{:?}", &game);
        game.round = 5; // force single hand
        while game.winner.is_none() {
            let mut iterations = 1000;
            let action = if game.current_player == 0 || game.current_player == 2 {
                let mut game = game.clone();
                game.experiment = false;
                iterations = 10;
                get_mcts_move(&game, iterations, false)
            } else {
                let mut game = game.clone();
                game.experiment = true;
                iterations = 2000;
                get_mcts_move(&game, iterations, false)
            };

            game.apply_move(action);
        }
        println!("{:?}", &game);
        println!("{:?} ", &game.scores)
    }
}
