use tricksterstable_rs::games::torchlit::{get_mcts_move, TorchlitGame};

fn main() {
    for _ in 0..100 {
        let mut game = TorchlitGame::new();
        game.round = 4;
        while game.winner.is_none() {
            // println!("moves: {:?}", game.get_moves());
            // println!("state: {:?}", game.state);
            let iterations = 1000;
            game.experiment = false;
            if game.current_player == 0 || game.current_player == 2 {
                game.experiment = true;
            }
            let action = get_mcts_move(&game, iterations, false);
            game.apply_move(action);
        }
        // for dungeon in 0..8 {
        //     println!("dungeon cards: {:?}", game.dungeon_cards[dungeon]);
        //     println!(
        //         "dungeon score: {:?}",
        //         game.dungeon_cards[dungeon]
        //             .iter()
        //             .map(|c| c.get_points())
        //             .sum::<i32>()
        //     );
        // }
        println!("winner: {:?}", game.winner);
        println!("scores: {:?}", game.scores);
    }
}
