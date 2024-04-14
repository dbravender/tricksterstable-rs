use std::io;

use tricksterstable_rs::games::dealers_dilemma::{
    get_mcts_move, print_card, print_suit, Game, State, BID_CARD_OFFSET, BID_TYPE_EASY,
    DEALER_SELECT_CARD, TRUMP_SELECT,
};

pub fn get_input(prompt: &str) -> String {
    println!("{}", prompt);
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_goes_into_input_above) => {}
        Err(_no_updates_is_fine) => {}
    }
    input.trim().to_string()
}

fn display_game(game: &Game) {
    for player in 0..3 {
        println!(
            "player {} tricks taken: {} bid: {} bid_cards: {} score: {}",
            player,
            game.tricks_taken[player],
            serde_json::to_string(&game.bids[player]).unwrap(),
            game.bid_cards[player]
                .iter()
                .flatten()
                .map(|c| print_card(*c, false))
                .collect::<Vec<_>>()
                .join(" "),
            game.scores[player]
        );
    }
    println!("lead_suit: {}", print_suit(game.lead_suit));
    println!("trump: {}", print_suit(game.trump_suit));
    println!(
        "current_hand:\n{}",
        game.hands[0]
            .iter()
            .map(|c| print_card(*c, true))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("---");
    println!(
        "current_trick: {}",
        game.current_trick
            .iter()
            .flatten()
            .map(|c| print_card(*c, false))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("---");
}

fn show_dealer_select(game: &Game) {
    println!(
        "---\nSelect card to hand (trump?)\n\n{}",
        game.dealer_select
            .iter()
            .map(|c| print_card(*c, false))
            .enumerate()
            .map(|(i, c)| format!("{}: {}", i, c))
            .collect::<Vec<_>>()
            .join("\n")
    );
    if game.get_moves().len() == 4 {
        println!(
            "{}",
            game.dealer_select
                .iter()
                .map(|c| print_card(*c, false))
                .enumerate()
                .map(|(i, c)| format!("{}: {} (no trump)", i + 2, c))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    println!("---\n");
}

fn show_moves(game: &Game) {
    println!("State: {:?}", game.state);
    match game.state {
        State::Play => println!(
            "{}",
            game.hands[0]
                .iter()
                .map(|c| print_card(*c, true))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        State::BidType => println!("0: easy\n1: top\n2: difference\n3: zero"),
        State::TrumpSelect => println!("0: trump, 1: no trump"),
        State::BidCard => println!(
            "{}",
            game.hands[0]
                .iter()
                .map(|c| print_card(*c, true))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        State::DealerSelect => show_dealer_select(game),
    }
}

fn main() {
    let mut game = Game::new();
    game.human_player[0] = true;
    display_game(&game);
    while game.winner.is_none() {
        let mut action: i32 = -1;
        if game.current_player == 0 {
            display_game(&game);
            show_moves(&game);
            while game.get_moves().iter().all(|x| x != &action) {
                let action_string = get_input("Move: ");
                action = action_string.parse::<i32>().unwrap();
                action += match game.state {
                    State::Play => 0,
                    State::BidCard => BID_CARD_OFFSET,
                    State::DealerSelect => DEALER_SELECT_CARD,
                    State::BidType => BID_TYPE_EASY,
                    State::TrumpSelect => TRUMP_SELECT,
                };
            }
        } else {
            action = get_mcts_move(&game, 250);
        }
        game = game.clone_and_apply_move(action);
    }
    display_game(&game);
}
