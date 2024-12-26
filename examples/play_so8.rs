use std::io;

use tricksterstable_rs::games::so8::{get_mcts_move, SixOfVIIIGame, State};

pub fn get_input(prompt: &str) -> String {
    println!("{}", prompt);
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_goes_into_input_above) => {}
        Err(_no_updates_is_fine) => {}
    }
    input.trim().to_string()
}

fn display_game(game: &SixOfVIIIGame) {
    for team in 0..2 {
        println!(
            "team {} tricks taken for team: {} score: {}",
            team,
            game.cards_taken[team].len() / 4,
            game.scores[team]
        );
    }
    let lead_suit = if let Some(card) = game.current_trick[game.lead_player] {
        card.suit.text_display().to_string()
    } else {
        "".to_string()
    };
    println!("lead_suit: {:?}", lead_suit);
    println!("trump: {:?}", game.current_trump);
    println!(
        "current_hand:\n{}",
        game.hands[0]
            .iter()
            .map(|c| c.text_display(true))
            .collect::<Vec<_>>()
            .join("\n")
    );
    println!("---");
    println!(
        "current_trick: {}",
        game.current_trick
            .iter()
            .flatten()
            .map(|c| c.text_display(false))
            .collect::<Vec<_>>()
            .join("\n")
    );
    println!("---");
}

fn show_moves(game: &SixOfVIIIGame) {
    println!("State: {:?}", game.state);
    match game.state {
        State::OptionallyPlayChurchOfEngland => {
            println!("0: Pass");
            println!("1: Play Church of England");
        }
        State::PassCard | State::Play => println!(
            "{}",
            game.hands[0]
                .iter()
                .map(|c| c.text_display(true))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}

fn main() {
    let mut game = SixOfVIIIGame::new();
    game.human_player = Some(0);
    display_game(&game);
    while game.winner.is_none() {
        let mut action: i32 = -1;
        if game.current_player == 0 {
            display_game(&game);
            show_moves(&game);
            while !game.get_moves().contains(&action) {
                let action_string = get_input("Move: ");
                action = action_string.parse::<i32>().unwrap();
            }
        } else {
            action = get_mcts_move(&game, 250, false);
        }
        game.apply_move(action);
    }
    display_game(&game);
}
