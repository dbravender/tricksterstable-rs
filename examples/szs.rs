use rand::{thread_rng, Rng};
use tricksterstable_rs::games::szs::{get_mcts_move, Game};

fn main() {
    let mut rnd = thread_rng();
    for _ in 0..1000 {
        let mut game = Game::new();
        game.dealer = rnd.gen_range(0..3);
        game.current_player = game.dealer;
        while game.scores == vec![0, 0, 0] {
            let iterations = 1000;
            game.experiment = if game.current_player == 0 {
                true
            } else {
                false
            };
            let action = get_mcts_move(&game, iterations);
            game.apply_move(action);
        }
        println!("{:?} ", &game.scores)
    }
}
