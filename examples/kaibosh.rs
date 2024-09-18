use tricksterstable_rs::games::kaibosh::{get_mcts_move, KaiboshGame};

fn main() {
    let mut game = KaiboshGame::new();
    let mut just_before_end = game.clone();
    while game.scores_this_hand == [0, 0] {
        println!("{:?}", game.get_moves());
        let action = get_mcts_move(&game, 1000);
        just_before_end = game.clone();
        game.apply_move(Some(action));
    }
    println!("{:?}", just_before_end);
    println!("{:?}", game);
}
