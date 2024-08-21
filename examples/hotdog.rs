use rand::{thread_rng, Rng};
use tricksterstable_rs::games::hotdog::{get_mcts_move, HotdogGame, State};

fn main() {
    let mut rnd = thread_rng();
    for _ in 0..1000 {
        let mut game = HotdogGame::new();
        game.dealer = if rnd.gen_range(0..100) > 50 { 0 } else { 1 };
        game.current_player = game.dealer;
        //println!("{:?}", &game);
        while game.scores == [0, 0] {
            let debug = match game.state {
                State::Bid => true,
                _ => false,
            };

            let iterations = 1000;
            let action = if game.current_player == 0 {
                let mut game = game.clone();
                game.experiment = false;
                get_mcts_move(&game, iterations, debug)
            } else {
                let mut game = game.clone();
                game.experiment = true;
                get_mcts_move(&game, iterations, debug)
            };

            game.apply_move(action);
        }
        //println!("{:?}", &game);
        println!("{:?} ", &game.scores)
    }
}
