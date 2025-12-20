/*
Game: 3 Tricky Pigs
Designers: Andrew Stiles and Steven Ungaro
BoardGameGeek: https://boardgamegeek.com/boardgame/441614/3-tricky-pigs
*/

const PLAYER_COUNT: usize = 4;
const HAND_SIZE: usize = 12;
const ROUNDS: usize = 4;

#[derive(Copy, Clone, Debug)]
enum Suit {
    Straw,
    Sticks,
    Bricks,
    Wolf,
    Huff,
    Puff,
}

enum Bid {
    /// Try to win 0 tricks
    Sleep,
    /// Win 2 tricks
    Play,
    /// Win 3 or more tricks
    Work,
    /// Win the most tricks of any player
    Eat,
}

enum State {
    /// Select one of the bid cards
    Bid,
    /// Standard must-follow trick taking
    Play,
}

#[derive(Copy, Clone, Debug)]
struct Card {
    value: i32,
    suit: Suit,
}

struct ThreeTrickyPigsGame {
    /// Whether or not the Wolf suit has been broken yet
    wolf_suit_broken: bool,
    /// Regular cards in a trick (indexed by player)
    current_trick_regular: [Option<Card>; PLAYER_COUNT],
    /// Huff cards in the current trick (indexed by player)
    current_trick_huff: [Option<Card>; PLAYER_COUNT],
    /// Puff cards in the current trick (indexed by player)
    curent_trick_puff: [Option<Card>; PLAYER_COUNT],
    /// Each player's hand
    hands: [Vec<Card>; PLAYER_COUNT],
}

fn deck() -> Vec<Card> {
    let distributions: Vec<(Suit, Vec<i32>)> = vec![
        (Suit::Straw, (1..=10).collect()),
        (Suit::Sticks, (1..=10).collect()),
        (Suit::Bricks, (21..=30).collect()),
        (
            Suit::Wolf,
            vec![0, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 31],
        ),
        (Suit::Huff, (1..=4).collect()),
        (Suit::Puff, (2..=5).collect()),
    ];
    let mut cards: Vec<Card> = vec![];
    for (suit, values) in &distributions {
        for value in values {
            cards.push(Card {
                value: *value,
                suit: *suit,
            });
        }
    }
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck_composition() {
        let d = deck();
        // Total card count
        assert_eq!(d.len(), 50);
    }
}
