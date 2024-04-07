use colored::Colorize;
use std::io;

use tricksterstable_rs::games::dealers_dilemma::{
    get_mcts_move, Card, Game, State, Suit, BID_CARD_OFFSET, BID_TYPE_EASY, DEALER_SELECT_CARD,
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

fn print_card(card: Card) -> String {
    let string = format!("{:<2}", card.value);
    let colored_string = match card.suit {
        Suit::Red => string.red(),
        Suit::Blue => string.blue(),
        Suit::Yellow => string.yellow(),
        Suit::Green => string.green(),
    };
    return format!("{}:{}", card.id, colored_string.to_string());
}

fn display_game(game: &Game) {
    for player in 0..3 {
        println!(
            "player {}\ntricks taken: {}\nbid: {}\nbid_cards: {}\nscore: {}\n",
            player,
            game.tricks_taken[player],
            serde_json::to_string(&game.bids[player]).unwrap(),
            serde_json::to_string(&game.bid_cards[player]).unwrap(),
            game.scores[player]
        );
    }
    println!("lead_suit: {:?}", game.lead_suit);
    println!("trump: {:?}", game.trump_suit);
    println!(
        "current_hand:\n{}",
        game.hands[0]
            .iter()
            .map(|c| print_card(*c))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("---");
    println!(
        "current_trick: {}",
        game.current_trick
            .iter()
            .flatten()
            .map(|c| print_card(*c))
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
            .map(|c| print_card(*c))
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
                .map(|c| print_card(*c))
                .enumerate()
                .map(|(i, c)| format!("{}: {} (no trump)", i + 2, c))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    println!("---\n");
}

fn show_moves(game: &Game) {
    match game.state {
        State::Play => println!(
            "{}",
            game.hands[0]
                .iter()
                .map(|c| print_card(*c))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        State::BidType => println!("0: easy\n1: top\n2: difference\n3: zero"),
        State::BidCard => println!(
            "{}",
            game.hands[0]
                .iter()
                .map(|c| print_card(*c))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        State::DealerSelect => show_dealer_select(game),
    }
}

fn main() {
    let mut game = Game::new();
    game.human_player[0] = true;
    game.no_changes = true;
    display_game(&game);
    while game.winner.is_none() {
        let mut action: i32 = -1;
        if game.current_player == 0 {
            show_moves(&game);
            while game.get_moves().iter().all(|x| x != &action) {
                let action_string = get_input("Move: ");
                action = action_string.parse::<i32>().unwrap();
                action += match game.state {
                    State::Play => 0,
                    State::BidCard => BID_CARD_OFFSET,
                    State::DealerSelect => DEALER_SELECT_CARD,
                    State::BidType => BID_TYPE_EASY,
                };
            }
        } else {
            action = get_mcts_move(&game);
        }
        game = game.clone_and_apply_move(action);
        display_game(&game);
    }
}
