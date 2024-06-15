use ismcts::IsmctsHandler;
use tricksterstable_rs::games::{yokai2p::get_mcts_move, yokai2p::Yokai2pGame};

fn main() {
    for _ in 0..1000 {
        let mut game = Yokai2pGame::new();
        //println!("{:?}", &game);
        while game.hand_scores == [0, 0] {
            let action = if game.current_player == 0 {
                let mut newgame = game.clone();
                newgame.experiment = true;
                get_mcts_move(&newgame, 500)
            } else {
                let mut newgame = game.clone();
                newgame.experiment = false;
                get_mcts_move(&newgame, 500)
            };

            game.apply_move(&action);
        }
        //println!("{:?}", &game);
        println!("{:?}", &game.scores)
    }
}
