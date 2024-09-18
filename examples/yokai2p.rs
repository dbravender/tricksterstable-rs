use ismcts::IsmctsHandler;
use tricksterstable_rs::games::{yokai2p::get_mcts_move, yokai2p::Yokai2pGame};

fn main() {
    for _ in 0..1000 {
        let mut game = Yokai2pGame::new();
        //println!("{:?}", &game);
        while game.scores == [0, 0] {
            let action = if game.current_player == 0 {
                get_mcts_move(&game, 500)
            } else {
                get_mcts_move(&game, 1000)
            };

            game.apply_move(&action);
        }
        //println!("{:?}", &game);
        println!("{:?}", &game.scores)
    }
}
