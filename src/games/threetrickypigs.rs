/*
Game: 3 Tricky Pigs
Designers: Andrew Stiles and Steven Ungaro
BoardGameGeek: https://boardgamegeek.com/boardgame/441614/3-tricky-pigs
*/

/*

Planned flow:
- Player can stage huff cards, puff cards and then a card from their hand
- As soon as a card is played from the hand the play is committed
- We'll play the actual card on top of the huff and puff cards
*/

const PLAYER_COUNT: usize = 4;
const HAND_SIZE: usize = 12;
const ROUNDS: usize = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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
    /// Each player's current bid
    bids: [Option<Bid>; PLAYER_COUNT],
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

/// Returns the player index that won the trick
fn trick_winner(
    lead_player: usize,
    trick_regular: [Option<Card>; PLAYER_COUNT],
    trick_huff: [Option<Card>; PLAYER_COUNT],
    trick_puff: [Option<Card>; PLAYER_COUNT],
) -> usize {
    let empty_card = Card {
        value: 0,
        suit: Suit::Bricks,
    };
    let mut winning_player = lead_player;
    let mut winning_value = trick_regular[lead_player].unwrap().value;
    let contains_wolf = trick_regular.iter().any(|c| c.unwrap().suit == Suit::Wolf);
    for offset in 0..PLAYER_COUNT {
        let current_player = (offset + lead_player) % PLAYER_COUNT;
        let card_value = trick_regular[current_player].unwrap().value
            + trick_huff[current_player].unwrap_or(empty_card).value
            + trick_puff[current_player].unwrap_or(empty_card).value;
        let winning = if contains_wolf {
            // When at least one wolf card is in the trick the highest card wins
            // (later played cards win ties)
            card_value >= winning_value
        } else {
            // When all cards are pig cards the lowest value card wins
            // (later played cards win ties)
            card_value <= winning_value
        };
        if winning {
            winning_value = card_value;
            winning_player = current_player;
        }
    }
    winning_player
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

    // Helper to create a card
    fn card(value: i32, suit: Suit) -> Option<Card> {
        Some(Card { value, suit })
    }

    // Helper for no huff/puff
    fn no_modifiers() -> [Option<Card>; PLAYER_COUNT] {
        [None; PLAYER_COUNT]
    }

    // Rulebook Example 6: No wolf, lowest card wins
    // Player 0 leads 5, Player 1 plays 10+5 puff, Player 2 plays 2
    // Player 2 wins with lowest value (2)
    #[test]
    fn test_no_wolf_lowest_wins() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(10, Suit::Straw),
            card(2, Suit::Straw),
            card(7, Suit::Straw),
        ];
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 2);
    }

    // Rulebook Example (wolf present): Player 0 leads 5, Player 1 plays 10+5 puff,
    // Player 2 can't follow and plays 14 (wolf), highest card wins
    // 10+5=15 beats 14, so Player 1 wins
    #[test]
    fn test_wolf_present_highest_wins() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(10, Suit::Straw),
            card(14, Suit::Wolf),
            card(8, Suit::Bricks),
        ];
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 1); // 10+5=15 is highest
    }

    // Rulebook Example 7: No wolf, off-suit doesn't matter for winner calculation
    // Player 0 leads 21, Player 1 plays 8 (off-suit)
    // In a 2-relevant-player scenario, 8 < 21, so Player 1 would win (lowest)
    #[test]
    fn test_no_wolf_lower_value_wins_regardless_of_suit() {
        let trick_regular = [
            card(21, Suit::Bricks),
            card(8, Suit::Straw),
            card(25, Suit::Bricks),
            card(28, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 1); // 8 is lowest
    }

    // Rulebook Example 8: Tie goes to last played card
    // Player 0 leads 5, Player 1 plays 28, Player 2 plays 8, Player 3 plays 8
    // No wolf, lowest wins. 5 is lowest, Player 0 wins.
    // (Separate test below covers the tie-breaker scenario)
    #[test]
    fn test_no_wolf_example_8() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(28, Suit::Straw),
            card(8, Suit::Straw),
            card(8, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0); // 5 is lowest, Player 0 wins
    }

    // Tie-breaker test: Two players have the same lowest value
    // No wolf, lowest wins. Two 5s tie, last played wins.
    #[test]
    fn test_no_wolf_tie_goes_to_last_played() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(28, Suit::Straw),
            card(8, Suit::Straw),
            card(5, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3); // Last played 5 wins the tie
    }

    // Wolf present, tie goes to last played (highest)
    #[test]
    fn test_wolf_present_tie_goes_to_last_played() {
        let trick_regular = [
            card(15, Suit::Wolf),
            card(10, Suit::Straw),
            card(15, Suit::Sticks),
            card(8, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 2); // Second 15 wins (last played among ties)
    }

    // Test huff modifier adds to value
    #[test]
    fn test_huff_adds_to_value() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(3, Suit::Straw),
            card(6, Suit::Straw),
            card(7, Suit::Straw),
        ];
        // Player 1 has 3, but +4 huff makes it 7
        let trick_huff = [None, card(4, Suit::Huff), None, None];

        let winner = trick_winner(0, trick_regular, trick_huff, no_modifiers());
        // Without huff: 3 wins (lowest). With huff: 3+4=7, so 5 is now lowest
        assert_eq!(winner, 0);
    }

    // Test huff and puff together
    #[test]
    fn test_huff_and_puff_combined() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(2, Suit::Straw),
            card(6, Suit::Straw),
            card(7, Suit::Straw),
        ];
        // Player 1 has 2, but +3 huff +4 puff makes it 9
        let trick_huff = [None, card(3, Suit::Huff), None, None];
        let trick_puff = [None, card(4, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, trick_huff, trick_puff);
        // 2+3+4=9, so 5 is now lowest
        assert_eq!(winner, 0);
    }

    // Test with non-zero lead player
    #[test]
    fn test_lead_player_not_zero() {
        let trick_regular = [
            card(10, Suit::Straw), // Player 0
            card(8, Suit::Straw),  // Player 1
            card(5, Suit::Straw),  // Player 2 (leads)
            card(9, Suit::Straw),  // Player 3
        ];

        // Player 2 leads, play order is 2, 3, 0, 1
        let winner = trick_winner(2, trick_regular, no_modifiers(), no_modifiers());
        // No wolf, lowest wins: 5 (P2), 9 (P3), 10 (P0), 8 (P1)
        // Lowest is 5, Player 2 wins
        assert_eq!(winner, 2);
    }

    // Test lead player wrap-around with tie
    #[test]
    fn test_lead_player_wraparound_tie() {
        let trick_regular = [
            card(5, Suit::Straw),  // Player 0
            card(8, Suit::Straw),  // Player 1
            card(5, Suit::Sticks), // Player 2 (leads)
            card(9, Suit::Straw),  // Player 3
        ];

        // Player 2 leads, play order is 2, 3, 0, 1
        // Tie between P2 (5) and P0 (5), P0 plays later, P0 wins
        let winner = trick_winner(2, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0);
    }

    // Wolf card with value 0 (special wolf card)
    #[test]
    fn test_wolf_zero_value() {
        let trick_regular = [
            card(0, Suit::Wolf),
            card(5, Suit::Straw),
            card(3, Suit::Sticks),
            card(2, Suit::Bricks),
        ];

        // Wolf present, highest wins. 5 is highest.
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 1);
    }

    // Wolf card with value 31 (highest wolf)
    #[test]
    fn test_wolf_31_value() {
        let trick_regular = [
            card(31, Suit::Wolf),
            card(30, Suit::Bricks),
            card(28, Suit::Bricks),
            card(25, Suit::Bricks),
        ];

        // Wolf present, highest wins. 31 is highest.
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0);
    }

    // Multiple wolves in trick
    #[test]
    fn test_multiple_wolves() {
        let trick_regular = [
            card(11, Suit::Wolf),
            card(15, Suit::Wolf),
            card(15, Suit::Wolf),
            card(12, Suit::Wolf),
        ];

        // All wolves, highest wins, tie goes to last played
        // P1 and P2 both have 15, P2 played later
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 2);
    }

    // Huff/puff with wolf - modifiers still apply
    #[test]
    fn test_wolf_with_modifiers() {
        let trick_regular = [
            card(14, Suit::Wolf),
            card(10, Suit::Straw),
            card(8, Suit::Sticks),
            card(7, Suit::Bricks),
        ];
        // Player 1 has 10 + 5 puff = 15, beats wolf 14
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 1);
    }

    // All same values, last player wins (no wolf)
    #[test]
    fn test_all_same_value_no_wolf() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(5, Suit::Sticks),
            card(5, Suit::Bricks),
            card(5, Suit::Straw),
        ];

        // All 5s, no wolf, last played wins (Player 3)
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3);
    }

    // All same values, last player wins (with wolf)
    #[test]
    fn test_all_same_value_with_wolf() {
        let trick_regular = [
            card(15, Suit::Wolf),
            card(15, Suit::Sticks),
            card(15, Suit::Bricks),
            card(15, Suit::Straw),
        ];

        // All 15s, wolf present, last played wins (Player 3)
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3);
    }
}
