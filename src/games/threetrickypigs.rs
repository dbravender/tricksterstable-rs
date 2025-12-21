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

#[derive(Copy, Clone)]
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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
    id: i32,
}

impl Card {
    fn is_puff(&self) -> bool {
        self.suit == Suit::Puff
    }
    fn is_huff(&self) -> bool {
        self.suit == Suit::Huff
    }
    fn is_regular(&self) -> bool {
        !self.is_huff() && !self.is_puff()
    }
}

struct ThreeTrickyPigsGame {
    /// Current game state
    state: State,
    /// Current player
    current_player: usize,
    /// Lead player for the current trick
    lead_player: usize,
    /// Whether or not the Wolf suit has been broken yet
    wolf_suit_broken: bool,
    /// Regular cards in a trick (indexed by player)
    current_trick_regular: [Option<Card>; PLAYER_COUNT],
    /// Huff cards in the current trick (indexed by player)
    current_trick_huff: [Option<Card>; PLAYER_COUNT],
    /// Puff cards in the current trick (indexed by player)
    current_trick_puff: [Option<Card>; PLAYER_COUNT],
    /// Each player's hand
    hands: [Vec<Card>; PLAYER_COUNT],
    /// Each player's current bid
    bids: [Option<Bid>; PLAYER_COUNT],
    /// Tricks won by each player this round
    tricks_won: [usize; PLAYER_COUNT],
    /// Current round (1-4)
    current_round: usize,
    /// Total scores for each player
    scores: [i32; PLAYER_COUNT],
}

impl ThreeTrickyPigsGame {
    /// Returns possible moves
    fn get_moves(&self) -> Vec<i32> {
        match self.state {
            // 4 bid options
            State::Bid => (0..=3).collect(),
            // Regular trick play
            State::Play => {
                let lead_suit = self.current_trick_regular[self.lead_player].map(|c| c.suit);
                let puff_played = self.current_trick_puff[self.current_player].is_some();
                let huff_played = self.current_trick_huff[self.current_player].is_some();
                let hand = &self.hands[self.current_player];
                let is_leading = lead_suit.is_none();

                // Find playable regular cards

                // Find all cards in the current lead suit
                let follow_suit_cards: Vec<&Card> = hand
                    .iter()
                    .filter(|c| lead_suit.map_or(false, |s| c.suit == s))
                    .collect();

                let playable_regular_cards: Vec<&Card> = if follow_suit_cards.is_empty() {
                    // No lead card or no cards in lead suit - any regular card in hand
                    // can be played (but wolves only if broken or not leading)
                    hand.iter()
                        .filter(|c| {
                            c.is_regular()
                                && (c.suit != Suit::Wolf || self.wolf_suit_broken || !is_leading)
                        })
                        .collect()
                } else {
                    // Must follow suit in 3 Tricky Pigs
                    follow_suit_cards
                };

                // Find playable huff and puff cards
                let playable_huff_and_puff_cards = hand
                    .iter()
                    .filter(|c| (!puff_played && c.is_puff()) || (!huff_played && c.is_huff()));

                // Return all playable cards
                playable_regular_cards
                    .into_iter()
                    .chain(playable_huff_and_puff_cards)
                    .map(|c| c.id)
                    .collect()
            }
        }
    }

    /// Apply a move to the game state
    fn apply_move(&mut self, card_id: i32) {
        // Validate move is legal
        let valid_moves = self.get_moves();
        if !valid_moves.contains(&card_id) {
            panic!(
                "Invalid move: {} not in valid moves {:?}",
                card_id, valid_moves
            );
        }

        match self.state {
            State::Bid => {
                // card_id 0-3 maps to bid variants
                let bid = match card_id {
                    0 => Bid::Sleep,
                    1 => Bid::Play,
                    2 => Bid::Work,
                    3 => Bid::Eat,
                    _ => panic!("Invalid bid"),
                };
                self.bids[self.current_player] = Some(bid);
                self.current_player = (self.current_player + 1) % PLAYER_COUNT;

                // If all players have bid, move to play state
                if self.bids.iter().all(|b| b.is_some()) {
                    self.state = State::Play;
                    // Reset current_player to lead_player for first trick
                    self.current_player = self.lead_player;
                }
            }
            State::Play => {
                let hand = &mut self.hands[self.current_player];

                // Find and remove the card from hand
                let card_index = hand.iter().position(|c| c.id == card_id).unwrap();
                let card = hand.remove(card_index);

                // Place card in appropriate trick slot
                if card.is_huff() {
                    self.current_trick_huff[self.current_player] = Some(card);
                } else if card.is_puff() {
                    self.current_trick_puff[self.current_player] = Some(card);
                } else {
                    // After a regular card (pig or wolf) is played the move is
                    // committed
                    self.current_trick_regular[self.current_player] = Some(card);

                    // Check if wolf was played when couldn't follow suit (breaks wolf)
                    if card.suit == Suit::Wolf && !self.wolf_suit_broken {
                        let lead_suit =
                            self.current_trick_regular[self.lead_player].map(|c| c.suit);
                        // Wolf is broken if player couldn't follow lead suit
                        // (lead_suit exists and player played wolf instead)
                        if lead_suit.is_some() && lead_suit != Some(Suit::Wolf) {
                            self.wolf_suit_broken = true;
                        }
                    }

                    // Advance to next player
                    self.current_player = (self.current_player + 1) % PLAYER_COUNT;

                    // Check if trick is complete (all players have played a regular card)
                    let trick_complete = self.current_trick_regular.iter().all(|c| c.is_some());

                    if trick_complete {
                        // Determine winner
                        let winner = trick_winner(
                            self.lead_player,
                            self.current_trick_regular,
                            self.current_trick_huff,
                            self.current_trick_puff,
                        );

                        // Award trick to winner
                        self.tricks_won[winner] += 1;

                        // Clear trick slots
                        self.current_trick_regular = [None; PLAYER_COUNT];
                        self.current_trick_huff = [None; PLAYER_COUNT];
                        self.current_trick_puff = [None; PLAYER_COUNT];

                        // Winner leads next trick
                        self.lead_player = winner;
                        self.current_player = winner;

                        // Check if round has ended (any player has no pig/wolf cards)
                        let round_ended = self
                            .hands
                            .iter()
                            .any(|hand| !hand.iter().any(|c| c.is_regular()));

                        if round_ended {
                            self.end_round();
                        }
                    }
                }
            }
        }
    }

    /// End the current round and calculate scores
    /// Note: Dealing new hands should be done separately
    fn end_round(&mut self) {
        // Calculate scores for each player
        for player in 0..PLAYER_COUNT {
            let tricks = self.tricks_won[player] as i32;

            // +1 per trick won
            self.scores[player] += tricks;

            // -1 per leftover huff/puff in hand
            let leftover_modifiers = self.hands[player]
                .iter()
                .filter(|c| c.is_huff() || c.is_puff())
                .count() as i32;
            self.scores[player] -= leftover_modifiers;

            // Bid bonuses
            if let Some(bid) = self.bids[player] {
                match bid {
                    Bid::Sleep => {
                        if tricks == 0 {
                            self.scores[player] += 12;
                        }
                    }
                    Bid::Play => {
                        if tricks == 2 {
                            self.scores[player] += 7;
                        }
                    }
                    Bid::Work => {
                        if tricks >= 3 {
                            self.scores[player] += 3;
                        }
                    }
                    Bid::Eat => {
                        // Check if this player won the most tricks
                        let max_tricks = self.tricks_won.iter().max().unwrap();
                        if self.tricks_won[player] == *max_tricks {
                            self.scores[player] += 2 * tricks;
                        }
                    }
                }
            }
        }

        // Advance to next round
        self.current_round += 1;

        // Reset for next round (if game not over)
        // Note: hands need to be dealt separately
        if self.current_round <= ROUNDS {
            self.tricks_won = [0; PLAYER_COUNT];
            self.bids = [None; PLAYER_COUNT];
            self.wolf_suit_broken = false;
            self.state = State::Bid;
            self.current_player = self.lead_player;
        }
    }

    /// Check if the game is over (4 rounds completed)
    fn is_game_over(&self) -> bool {
        self.current_round > ROUNDS
    }
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
    let mut id = 0;
    let mut cards: Vec<Card> = vec![];
    for (suit, values) in &distributions {
        for value in values {
            cards.push(Card {
                value: *value,
                suit: *suit,
                id: id,
            });
            id += 1;
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
        id: -1,
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
        Some(Card {
            id: -1,
            value,
            suit,
        })
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

    // ==================== get_moves tests ====================

    // Helper to create a card with id
    fn card_with_id(id: i32, value: i32, suit: Suit) -> Card {
        Card { id, value, suit }
    }

    // Helper to create a game with specific state
    fn game_with_hand(
        current_player: usize,
        lead_player: usize,
        hand: Vec<Card>,
        trick_regular: [Option<Card>; PLAYER_COUNT],
        trick_huff: [Option<Card>; PLAYER_COUNT],
        trick_puff: [Option<Card>; PLAYER_COUNT],
        wolf_suit_broken: bool,
    ) -> ThreeTrickyPigsGame {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[current_player] = hand;
        ThreeTrickyPigsGame {
            state: State::Play,
            current_player,
            lead_player,
            wolf_suit_broken,
            current_trick_regular: trick_regular,
            current_trick_huff: trick_huff,
            current_trick_puff: trick_puff,
            hands,
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        }
    }

    // Bid state returns all 4 bid options
    #[test]
    fn test_get_moves_bid_state() {
        let game = ThreeTrickyPigsGame {
            state: State::Bid,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: false,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands: Default::default(),
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };
        let moves = game.get_moves();
        assert_eq!(moves, vec![0, 1, 2, 3]);
    }

    // Leading player (no lead card) can play any card
    #[test]
    fn test_get_moves_leading_player_any_card() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 10, Suit::Sticks),
            card_with_id(2, 25, Suit::Bricks),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // All cards playable when leading
        assert!(moves.contains(&0));
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
    }

    // Must follow suit when able
    #[test]
    fn test_get_moves_must_follow_suit() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 7, Suit::Straw),
            card_with_id(2, 25, Suit::Bricks),
        ];

        // Player 0 led with Straw
        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // Only Straw cards playable
        assert!(moves.contains(&0)); // Straw
        assert!(moves.contains(&1)); // Straw
        assert!(!moves.contains(&2)); // Bricks - can't play
    }

    // Can play any card if can't follow suit
    #[test]
    fn test_get_moves_cant_follow_suit() {
        let hand = vec![
            card_with_id(0, 25, Suit::Bricks),
            card_with_id(1, 26, Suit::Bricks),
            card_with_id(2, 15, Suit::Wolf),
        ];

        // Player 0 led with Straw
        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // No Straw in hand, can play anything
        assert!(moves.contains(&0));
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
    }

    // Huff cards are playable when not already played
    #[test]
    fn test_get_moves_huff_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card
        assert!(moves.contains(&1)); // huff card
    }

    // Huff cards not playable when already played this trick
    #[test]
    fn test_get_moves_huff_already_played() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let mut trick_huff = no_modifiers();
        trick_huff[0] = Some(Card {
            id: 99,
            value: 1,
            suit: Suit::Huff,
        });

        let game = game_with_hand(0, 0, hand, no_modifiers(), trick_huff, no_modifiers(), true);
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card still playable
        assert!(!moves.contains(&1)); // huff card NOT playable
    }

    // Puff cards are playable when not already played
    #[test]
    fn test_get_moves_puff_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card
        assert!(moves.contains(&1)); // puff card
    }

    // Puff cards not playable when already played this trick
    #[test]
    fn test_get_moves_puff_already_played() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let mut trick_puff = no_modifiers();
        trick_puff[0] = Some(Card {
            id: 99,
            value: 2,
            suit: Suit::Puff,
        });

        let game = game_with_hand(0, 0, hand, no_modifiers(), no_modifiers(), trick_puff, true);
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card still playable
        assert!(!moves.contains(&1)); // puff card NOT playable
    }

    // Both huff and puff playable
    #[test]
    fn test_get_moves_huff_and_puff_both_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
            card_with_id(2, 3, Suit::Puff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular
        assert!(moves.contains(&1)); // huff
        assert!(moves.contains(&2)); // puff
    }

    // Following player with huff/puff in hand, must follow suit for regular
    #[test]
    fn test_get_moves_follow_suit_with_huff_puff() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 25, Suit::Bricks),
            card_with_id(2, 2, Suit::Huff),
            card_with_id(3, 3, Suit::Puff),
        ];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - must follow
        assert!(!moves.contains(&1)); // Bricks - can't play
        assert!(moves.contains(&2)); // Huff - always playable if not used
        assert!(moves.contains(&3)); // Puff - always playable if not used
    }

    // Player 2 following, player 0 led
    #[test]
    fn test_get_moves_different_lead_and_current_player() {
        let hand = vec![
            card_with_id(0, 5, Suit::Sticks),
            card_with_id(1, 25, Suit::Bricks),
        ];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Sticks,
        });
        trick_regular[1] = Some(Card {
            id: 98,
            value: 7,
            suit: Suit::Sticks,
        });

        let game = game_with_hand(
            2,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Sticks - follows suit
        assert!(!moves.contains(&1)); // Bricks - can't play
    }

    // Cannot lead wolf when wolf not broken
    #[test]
    fn test_get_moves_cannot_lead_wolf_when_not_broken() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 15, Suit::Wolf),
        ];

        // Leading (no cards played), wolf not broken
        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            false,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - can lead
        assert!(!moves.contains(&1)); // Wolf - cannot lead when not broken
    }

    // Can lead wolf when wolf is broken
    #[test]
    fn test_get_moves_can_lead_wolf_when_broken() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 15, Suit::Wolf),
        ];

        // Leading (no cards played), wolf IS broken
        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - can lead
        assert!(moves.contains(&1)); // Wolf - can lead when broken
    }

    // ==================== apply_move tests ====================

    // Helper to create a game in bid state
    fn game_in_bid_state() -> ThreeTrickyPigsGame {
        ThreeTrickyPigsGame {
            state: State::Bid,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: false,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands: Default::default(),
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        }
    }

    // Bidding advances current player
    #[test]
    fn test_apply_move_bid_advances_player() {
        let mut game = game_in_bid_state();
        assert_eq!(game.current_player, 0);

        game.apply_move(0); // Player 0 bids Sleep
        assert_eq!(game.current_player, 1);

        game.apply_move(1); // Player 1 bids Play
        assert_eq!(game.current_player, 2);
    }

    // All bids complete transitions to Play state
    #[test]
    fn test_apply_move_bid_transitions_to_play() {
        let mut game = game_in_bid_state();

        game.apply_move(0); // Player 0
        game.apply_move(1); // Player 1
        game.apply_move(2); // Player 2
        assert!(matches!(game.state, State::Bid)); // Still bidding

        game.apply_move(3); // Player 3
        assert!(matches!(game.state, State::Play)); // Now playing
    }

    // Playing a regular card removes it from hand
    #[test]
    fn test_apply_move_removes_card_from_hand() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 10, Suit::Sticks),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        assert_eq!(game.hands[0].len(), 2);
        game.apply_move(0); // Play Straw card
        assert_eq!(game.hands[0].len(), 1);
        assert_eq!(game.hands[0][0].id, 1); // Only Sticks card remains
    }

    // Playing a regular card places it in trick
    #[test]
    fn test_apply_move_places_regular_card_in_trick() {
        let hand = vec![card_with_id(0, 5, Suit::Straw)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(0);
        assert!(game.current_trick_regular[0].is_some());
        assert_eq!(game.current_trick_regular[0].unwrap().id, 0);
    }

    // Playing a huff card places it in huff slot
    #[test]
    fn test_apply_move_places_huff_card() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(1); // Play huff
        assert!(game.current_trick_huff[0].is_some());
        assert_eq!(game.current_trick_huff[0].unwrap().id, 1);
        // Player doesn't advance after huff
        assert_eq!(game.current_player, 0);
    }

    // Playing a puff card places it in puff slot
    #[test]
    fn test_apply_move_places_puff_card() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(1); // Play puff
        assert!(game.current_trick_puff[0].is_some());
        assert_eq!(game.current_trick_puff[0].unwrap().id, 1);
        // Player doesn't advance after puff
        assert_eq!(game.current_player, 0);
    }

    // Regular card advances to next player
    #[test]
    fn test_apply_move_regular_card_advances_player() {
        let hand = vec![card_with_id(0, 5, Suit::Straw)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        assert_eq!(game.current_player, 0);
        game.apply_move(0);
        assert_eq!(game.current_player, 1);
    }

    // Complete trick determines winner and clears slots
    #[test]
    fn test_apply_move_complete_trick() {
        // Set up a 4-player trick where player 0 leads
        // Each player has an extra card so round doesn't end
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 5, Suit::Straw), card_with_id(10, 1, Suit::Sticks)];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw), card_with_id(11, 2, Suit::Sticks)]; // Lowest - will win
        hands[2] = vec![card_with_id(2, 7, Suit::Straw), card_with_id(12, 3, Suit::Sticks)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw), card_with_id(13, 4, Suit::Sticks)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        game.apply_move(0); // Player 0 plays 5
        game.apply_move(1); // Player 1 plays 3
        game.apply_move(2); // Player 2 plays 7
        game.apply_move(3); // Player 3 plays 9

        // Player 1 wins (lowest card, no wolf)
        assert_eq!(game.tricks_won[1], 1);
        assert_eq!(game.lead_player, 1);
        assert_eq!(game.current_player, 1);

        // Trick slots cleared
        assert!(game.current_trick_regular.iter().all(|c| c.is_none()));
    }

    // Wolf breaks when played because can't follow suit
    #[test]
    fn test_apply_move_wolf_breaks() {
        // Player 0 leads Straw, Player 1 has no Straw so plays Wolf
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[1] = vec![card_with_id(1, 15, Suit::Wolf)];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 0,
            value: 5,
            suit: Suit::Straw,
        });

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 1,
            lead_player: 0,
            wolf_suit_broken: false,
            current_trick_regular: trick_regular,
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        assert!(!game.wolf_suit_broken);
        game.apply_move(1); // Player 1 plays Wolf
        assert!(game.wolf_suit_broken);
    }

    // Wolf doesn't break when leading (after already broken)
    #[test]
    fn test_apply_move_wolf_lead_doesnt_rebreak() {
        let hand = vec![card_with_id(0, 15, Suit::Wolf)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true, // Already broken
        );

        game.apply_move(0); // Lead with Wolf
        assert!(game.wolf_suit_broken); // Still broken (unchanged)
    }

    // Invalid move panics
    #[test]
    #[should_panic(expected = "Invalid move")]
    fn test_apply_move_invalid_move_panics() {
        let hand = vec![card_with_id(0, 5, Suit::Straw)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(99); // Invalid card id
    }

    // Huff then puff then regular card sequence
    #[test]
    fn test_apply_move_huff_puff_regular_sequence() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
            card_with_id(2, 3, Suit::Puff),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        // Play huff - stays on same player
        game.apply_move(1);
        assert_eq!(game.current_player, 0);
        assert!(game.current_trick_huff[0].is_some());

        // Play puff - stays on same player
        game.apply_move(2);
        assert_eq!(game.current_player, 0);
        assert!(game.current_trick_puff[0].is_some());

        // Play regular - advances player
        game.apply_move(0);
        assert_eq!(game.current_player, 1);
        assert!(game.current_trick_regular[0].is_some());
    }

    // Trick winner with huff/puff modifiers
    #[test]
    fn test_apply_move_trick_winner_with_modifiers() {
        // Each player has an extra regular card so round doesn't end
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 2, Suit::Straw),
            card_with_id(10, 4, Suit::Huff),
            card_with_id(20, 5, Suit::Sticks), // Extra regular card
        ];
        hands[1] = vec![card_with_id(1, 8, Suit::Straw), card_with_id(11, 1, Suit::Sticks)];
        hands[2] = vec![card_with_id(2, 9, Suit::Straw), card_with_id(12, 2, Suit::Sticks)];
        hands[3] = vec![card_with_id(3, 7, Suit::Straw), card_with_id(13, 3, Suit::Sticks)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        // Player 0 plays huff (+4) then 2 = 6 total
        game.apply_move(10); // Huff
        game.apply_move(0); // 2 Straw (total 6)
        game.apply_move(1); // Player 1: 8
        game.apply_move(2); // Player 2: 9
        game.apply_move(3); // Player 3: 7

        // Player 0 wins with 6 (2 base + 4 huff), lowest value
        assert_eq!(game.tricks_won[0], 1);
    }

    // ==================== Round end tests ====================

    // Round ends when any player has no regular cards
    #[test]
    fn test_round_ends_when_player_has_no_regular_cards() {
        // Each player has exactly one card - round ends after one trick
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 5, Suit::Straw)];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Work); PLAYER_COUNT], // Everyone bid Work
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        assert_eq!(game.current_round, 1);

        // Play the trick
        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Round should have ended and advanced
        assert_eq!(game.current_round, 2);
        assert_eq!(game.state, State::Bid);
    }

    // Scoring: +1 per trick won
    #[test]
    fn test_scoring_tricks_won() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 5, Suit::Straw)];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)]; // Wins
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 1 won 1 trick, gets +1 point
        assert_eq!(game.scores[1], 1);
        assert_eq!(game.scores[0], 0);
    }

    // Scoring: -1 per leftover huff/puff
    #[test]
    fn test_scoring_leftover_modifiers() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(10, 2, Suit::Huff),
            card_with_id(11, 3, Suit::Puff),
        ];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        game.apply_move(0); // Player 0 plays regular, keeps huff and puff
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 0 has 2 leftover modifiers: -2 points
        assert_eq!(game.scores[0], -2);
    }

    // Scoring: Sleep bid (+12 if 0 tricks)
    #[test]
    fn test_scoring_sleep_bid_success() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 10, Suit::Straw)]; // High card, won't win
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)]; // Wins
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Sleep), None, None, None],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 0 won 0 tricks with Sleep bid: +12 points
        assert_eq!(game.scores[0], 12);
    }

    // Scoring: Play bid (+7 if exactly 2 tricks)
    #[test]
    fn test_scoring_play_bid_success() {
        // Set up for 2 tricks where player 0 wins both
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 1, Suit::Straw),
            card_with_id(4, 1, Suit::Sticks),
        ];
        hands[1] = vec![
            card_with_id(1, 5, Suit::Straw),
            card_with_id(5, 5, Suit::Sticks),
        ];
        hands[2] = vec![
            card_with_id(2, 6, Suit::Straw),
            card_with_id(6, 6, Suit::Sticks),
        ];
        hands[3] = vec![
            card_with_id(3, 7, Suit::Straw),
            card_with_id(7, 7, Suit::Sticks),
        ];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Play), None, None, None],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        // Trick 1
        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Trick 2
        game.apply_move(4);
        game.apply_move(5);
        game.apply_move(6);
        game.apply_move(7);

        // Player 0 won 2 tricks with Play bid: 2 + 7 = 9 points
        assert_eq!(game.scores[0], 9);
    }

    // Scoring: Work bid (+3 if 3+ tricks)
    #[test]
    fn test_scoring_work_bid_success() {
        // Set up for 3 tricks where player 0 wins all
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 1, Suit::Straw),
            card_with_id(4, 1, Suit::Sticks),
            card_with_id(8, 21, Suit::Bricks),
        ];
        hands[1] = vec![
            card_with_id(1, 5, Suit::Straw),
            card_with_id(5, 5, Suit::Sticks),
            card_with_id(9, 25, Suit::Bricks),
        ];
        hands[2] = vec![
            card_with_id(2, 6, Suit::Straw),
            card_with_id(6, 6, Suit::Sticks),
            card_with_id(10, 26, Suit::Bricks),
        ];
        hands[3] = vec![
            card_with_id(3, 7, Suit::Straw),
            card_with_id(7, 7, Suit::Sticks),
            card_with_id(11, 27, Suit::Bricks),
        ];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Work), None, None, None],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        // Play 3 tricks
        for _ in 0..3 {
            let moves = game.get_moves();
            game.apply_move(moves[0]);
            let moves = game.get_moves();
            game.apply_move(moves[0]);
            let moves = game.get_moves();
            game.apply_move(moves[0]);
            let moves = game.get_moves();
            game.apply_move(moves[0]);
        }

        // Player 0 won 3 tricks with Work bid: 3 + 3 = 6 points
        assert_eq!(game.scores[0], 6);
    }

    // Scoring: Eat bid (+2 per trick if most tricks)
    #[test]
    fn test_scoring_eat_bid_success() {
        // Player 0 wins the only trick
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 1, Suit::Straw)]; // Lowest, wins
        hands[1] = vec![card_with_id(1, 5, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 6, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 7, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Eat), None, None, None],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            scores: [0; PLAYER_COUNT],
        };

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 0 won 1 trick (most) with Eat bid: 1 + 2*1 = 3 points
        assert_eq!(game.scores[0], 3);
    }

    // Game ends after 4 rounds
    #[test]
    fn test_game_ends_after_four_rounds() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 5, Suit::Straw)];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 4, // Last round
            scores: [0; PLAYER_COUNT],
        };

        assert!(!game.is_game_over());

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        assert!(game.is_game_over());
        assert_eq!(game.current_round, 5);
    }
}
