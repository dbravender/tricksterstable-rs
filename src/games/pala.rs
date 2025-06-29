/*
Game: Pala
Designer: Jeffrey Allers
BoardGameGeek: https://boardgamegeek.com/boardgame/37441/pala
*/

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde::{Deserialize, Serialize};

const HAND_SIZE: usize = 11;
const PASS_BID: i32 = -1;
// Used when selecting where to play cards (smearing, initial play)
const PLAY_OFFSET: i32 = -50; // -50 0 player offset, -51 1 player offset, etc.
const UNDO: i32 = -2; // Only the human player can undo their moves
const CHOOSE_TO_WIN: i32 = -20; // Player chose to win after playing a tying card
const CHOOSE_TO_LOSE: i32 = -21; // Player chose to lose after playing a tying card
const SKIP_MIX: i32 = -200; // Player could mix but chooses not to
const BID_OFFSET: i32 = -10; // -10 first bid slot, -9 second bid slot, etc.
const LOCATION_BASED_MOVES: &[i32] = &[
    PLAY_OFFSET,
    PLAY_OFFSET + 1,
    PLAY_OFFSET + 2,
    PLAY_OFFSET + 3,
    UNDO,
    CHOOSE_TO_WIN,
    CHOOSE_TO_LOSE,
    SKIP_MIX,
    BID_OFFSET,
    BID_OFFSET + 1,
    BID_OFFSET + 2,
    BID_OFFSET + 3,
];
const PLAYER_COUNT: usize = 4;
const POINT_THRESHOLD: i32 = 45;
const BID_CARDS: [BidSpace; PLAYER_COUNT] = [
    BidSpace::PlusFace,
    BidSpace::PlusOne,
    BidSpace::PlusOne,
    BidSpace::Cancel,
];

#[derive(
    Debug, Clone, Serialize, Sequence, Deserialize, PartialEq, Eq, Copy, Hash, PartialOrd, Ord,
)]
pub enum BidSpace {
    PlusFace,
    PlusOne,
    Cancel,
    Missing,
}

impl BidSpace {
    pub fn score_for_card(&self, card: &Card) -> i32 {
        match self {
            BidSpace::PlusFace => card.value,
            BidSpace::PlusOne => 1,
            // During scoring, highest value cards will automatically be cancelled and remaining
            // cancel cards are worth -1
            BidSpace::Cancel => -1,
            // Suits which were not bid score as 0
            BidSpace::Missing => 0,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    Serialize,
    Sequence,
    Deserialize,
    PartialEq,
    Eq,
    Copy,
    Hash,
    PartialOrd,
    Ord,
)]
#[serde(rename_all = "camelCase")]
pub enum Suit {
    #[default]
    Red = 0,
    Orange = 1,
    Yellow = 2,
    Green = 3,
    Blue = 4,
    Purple = 5,
}

impl Suit {
    pub fn is_primary(&self) -> bool {
        match self {
            Suit::Blue | Suit::Red | Suit::Yellow => true,
            _ => false,
        }
    }

    pub fn is_secondary(&self) -> bool {
        !self.is_primary()
    }

    pub fn composed_of(&self) -> [Suit; 2] {
        match self {
            Suit::Green => [Suit::Blue, Suit::Yellow],
            Suit::Purple => [Suit::Blue, Suit::Red],
            Suit::Orange => [Suit::Yellow, Suit::Red],
            _ => panic!("primary colors are not composed of other colors"),
        }
    }

    pub fn mixed_with(&self, other: Suit) -> Suit {
        match (self, other) {
            (Suit::Blue, Suit::Yellow) => Suit::Green,
            (Suit::Yellow, Suit::Blue) => Suit::Green,
            (Suit::Red, Suit::Blue) => Suit::Purple,
            (Suit::Blue, Suit::Red) => Suit::Purple,
            (Suit::Yellow, Suit::Red) => Suit::Orange,
            (Suit::Red, Suit::Yellow) => Suit::Orange,
            _ => panic!("secondary colors cannot be mixed"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    id: i32,
    pub suit: Suit,
    value: i32,
}

impl Card {
    fn ties(&self, other: Card) -> bool {
        self.suit == other.suit && self.value == other.value
    }

    fn beats(&self, other: Card) -> bool {
        self.suit == other.suit && self.value > other.value
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    // Select a card from hand to play to bid (or pass)
    BidSelectBidCard,
    // Select a bid location
    BidSelectBidLocation,
    // Select a card to play (as a smear, mix, follow, or junk)
    SelectCardToPlay,
    // Select location to play card
    SelectLocationToPlay,
    // Select whether or not the current player is winning (when a tying card is played)
    SelectWinningOrLosing,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Location {
    #[default]
    Deck,
    Hand,
    Play,
    TricksTaken,
    Score,
    ReorderHand,
    Message,
    BurnCards,
    PlayCombine,
    SpawnNewCard,
    DeleteCard,
    Bid,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    Faceup,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    Reorder,
    Message,
    BurnCards,
    DeleteCard,
    Bid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    object_id: i32,
    dest: Location,
    startscore: i32,
    end_score: i32,
    offset: usize,
    player: usize,
    length: usize,
    highlight: bool,
    message: Option<String>,
    card: Option<Card>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PalaGame {
    // Current game state
    pub state: State,
    // Which player is making a move now
    pub current_player: usize, // 0 - 3
    // Player who led the current trick
    pub lead_player: usize,
    // Cards each player has played in the current trick (includes spawned smeared and mixed cards)
    pub current_trick: [Option<Card>; PLAYER_COUNT],
    // Actual cards in the trick
    pub actual_trick_cards: Vec<Card>,
    // Cards in each player's hand
    pub hands: [Vec<Card>; PLAYER_COUNT],
    // Voids revealed when a player couldn't follow a lead card (used during determination)
    pub voids: [Vec<Suit>; PLAYER_COUNT],
    // Player who starts the next hand
    pub dealer: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    pub changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    pub no_changes: bool,
    // Current score of the game
    pub scores: [i32; PLAYER_COUNT],
    // Game winner
    pub winner: Option<usize>,
    // Use experimental reward function for comparison
    pub experiment: bool,
    // Which player is the human player
    pub human_player: Option<usize>,
    // Cards played on the bid spaces
    pub bids: [Option<Suit>; PLAYER_COUNT],
    // Denormalized map of suit to BidSpace
    pub suit_to_bid: HashMap<Suit, BidSpace>,
    // Cards won by each player
    pub cards_won: [Vec<Card>; PLAYER_COUNT],
    // Card selected for moves that require multiple actions
    pub selected_card: Option<Card>,
    // Tracks which player is winning - in Pala ties are possible
    // and the winning player is selected by the player who played
    // the tying card
    // Additionally, players' cards can be smeared by opponents
    // causing a player who played a non-winning card to become the winner
    // so we will recalculate the winner after each play
    pub trick_winning_player: usize,
    // When cards are smeared or mixed a new card object is spawned
    // to track animations
    pub next_id: i32,
}

impl PalaGame {
    pub fn new() -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        let mut rng = rand::thread_rng();
        game.dealer = rng.gen_range(0..PLAYER_COUNT);
        game.deal();
        game
    }

    pub fn new_with_human_player(human_player: usize) -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        let mut rng = rand::thread_rng();
        game.dealer = rng.gen_range(0..PLAYER_COUNT);
        game.human_player = Some(human_player);
        game.deal();
        game
    }

    // Called at the start of a game and when a new hand is dealt
    pub fn deal(&mut self) {
        self.state = State::BidSelectBidCard;
        self.bids = [None; PLAYER_COUNT];
        self.hands = [vec![], vec![], vec![], vec![]];
        self.current_trick = [None; PLAYER_COUNT];
        self.dealer = (self.dealer + 1) % PLAYER_COUNT;
        self.current_player = self.dealer;
        self.lead_player = self.current_player;
        self.next_id = 100;
        // By definition the lead player will start out winning the trick
        self.trick_winning_player = self.current_player;
        self.voids = [vec![], vec![], vec![], vec![]];
        let mut cards = PalaGame::deck();
        let shuffle_index = self.new_change();
        let deal_index = self.new_change();
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::Shuffle,
                object_id: 0,
                dest: Location::Deck,
                ..Default::default()
            },
        );
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::Message,
                message: Some("".to_string()),
                ..Default::default()
            },
        );
        for hand_index in 0..HAND_SIZE {
            for player in 0..PLAYER_COUNT {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        player,
                        offset: hand_index,
                        length: HAND_SIZE,
                        ..Default::default()
                    },
                );
                self.hands[player].push(card);
            }
        }
        for card in cards {
            self.add_change(
                deal_index,
                Change {
                    change_type: ChangeType::BurnCards,
                    object_id: card.id,
                    dest: Location::BurnCards,
                    ..Default::default()
                },
            );
        }
        for player in 0..PLAYER_COUNT {
            self.sort_hand(player);
            self.reorder_hand(player, player == 0);
        }
        self.show_playable();
        self.show_message();
    }

    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        for suit in all::<Suit>() {
            let values = if suit.is_primary() {
                [1, 1, 2, 2, 3, 3, 4, 5]
            } else {
                [2, 3, 4, 5, 6, 7, 8, 9]
            };
            for value in values {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        return deck;
    }

    fn peek_card(&mut self, id: i32) -> Card {
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == id)
            .unwrap();
        return self.hands[self.current_player][pos];
    }

    fn pop_card(&mut self, id: i32) -> Card {
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == id)
            .unwrap();
        let card = self.hands[self.current_player].remove(pos);
        return card;
    }

    // Intended to be called when all bids are finished
    pub fn set_suit_to_bid(&mut self) {
        self.suit_to_bid = HashMap::new();
        for i in 0..PLAYER_COUNT {
            let suit = self.bids[i].unwrap();
            self.suit_to_bid.insert(suit, BID_CARDS[i]);
        }
    }

    pub fn get_moves(&self) -> Vec<i32> {
        match self.state {
            State::BidSelectBidCard => self.get_moves_select_bid_card(),
            State::BidSelectBidLocation => self.get_moves_select_bid_location(),
            State::SelectCardToPlay => self.get_playable_cards(),
            State::SelectLocationToPlay => self.get_locations_to_play(),
            State::SelectWinningOrLosing => vec![CHOOSE_TO_WIN, CHOOSE_TO_LOSE],
        }
    }

    fn get_moves_select_bid_card(&self) -> Vec<i32> {
        let bid_suits: HashSet<Suit> = self.bids.iter().flat_map(|&s| s).collect();
        let mut options: Vec<i32> = self.hands[self.current_player]
            .iter()
            .filter(|c| !bid_suits.contains(&c.suit))
            .map(|c| c.id)
            .collect();

        options.push(PASS_BID);
        options
    }

    fn get_moves_select_bid_location(&self) -> Vec<i32> {
        let mut bid_locations: Vec<i32> = self
            .bids
            .iter()
            .enumerate()
            .filter_map(|(i, bid)| bid.is_none().then(|| BID_OFFSET + i as i32))
            .collect();
        if self.human_player == Some(self.current_player) {
            bid_locations.push(UNDO);
        }
        return bid_locations;
    }

    fn get_playable_cards(&self) -> Vec<i32> {
        if !self.cards_playable_as_a_mix().is_empty() {
            let mut plays: Vec<i32> = self
                .cards_playable_as_a_mix()
                .iter()
                .map(|&c| c.id)
                .collect();
            // FIXME - only allow skipping when the suit of the winning card
            // is not the suit that is being mixed to
            plays.push(SKIP_MIX);
            if self.human_player == Some(self.current_player) {
                plays.push(UNDO);
            }
            return plays;
        }
        let lead_suit = self.get_lead_suit();
        if lead_suit.is_some() {
            let lead_suit = lead_suit.unwrap();
            let mut moves: Vec<i32> = self.hands[self.current_player]
                .iter()
                .filter(|c| c.suit == lead_suit)
                .map(|c| c.id)
                .collect();
            let mut mixables = self.get_mixable_card_ids(lead_suit);
            moves.append(&mut mixables);
            if !moves.is_empty() {
                return moves;
            }
        }
        return self.current_player_card_ids();
    }

    fn cards_playable_as_a_smear(&self) -> Vec<Card> {
        if self.current_player == self.trick_winning_player {
            return vec![];
        }
        let winning_suit = self.current_trick[self.trick_winning_player].unwrap().suit;
        if winning_suit.is_secondary() {
            return vec![];
        }
        let smearable_suits: HashSet<Suit> = self.hands[self.current_player]
            .iter()
            .filter(|c| c.suit.is_secondary())
            .map(|c| c.suit)
            .collect();

        self.hands[self.current_player]
            .iter()
            .filter(|c| {
                c.suit.is_primary()
                    && c.suit != winning_suit
                    && smearable_suits.contains(&c.suit.mixed_with(winning_suit))
            })
            .map(|c| *c)
            .collect()
    }

    fn get_mixable_card_ids(&self, target_suit: Suit) -> Vec<i32> {
        if target_suit.is_primary() {
            return vec![];
        }
        let mixing_suits = target_suit.composed_of();
        self.hands[self.current_player]
            .iter()
            .filter(|c| mixing_suits[0] == c.suit || mixing_suits[1] == c.suit)
            .map(|c| c.id)
            .collect()
    }

    fn cards_playable_as_a_mix(&self) -> Vec<Card> {
        // Cannot play a mix when leading
        if self.current_trick[self.trick_winning_player].is_none()
            || self.current_trick[self.current_player].is_none()
        {
            return vec![];
        }
        let winning_suit = self.current_trick[self.trick_winning_player].unwrap().suit;
        let base_suit = self.current_trick[self.current_player].unwrap().suit;
        if winning_suit.is_primary() || base_suit.is_secondary() {
            return vec![];
        }

        for suit in [Suit::Red, Suit::Blue, Suit::Yellow] {
            if suit == base_suit {
                continue;
            }
            let mixable_suit = base_suit.mixed_with(suit);
            if mixable_suit == winning_suit {
                return self.hands[self.current_player]
                    .iter()
                    .filter(|c| c.suit == suit)
                    .map(|c| *c)
                    .collect();
            }
        }

        return vec![];
    }

    fn get_locations_to_play(&self) -> Vec<i32> {
        if self.lead_player == self.current_player {
            return vec![PLAY_OFFSET + self.current_player as i32];
        }
        let mut locations = vec![PLAY_OFFSET + self.current_player as i32];
        if self.human_player == Some(self.current_player) {
            locations.push(UNDO);
        }
        let selected_card = self.selected_card.unwrap();
        if self.cards_playable_as_a_smear().contains(&selected_card) {
            locations.insert(0, PLAY_OFFSET + self.trick_winning_player as i32);
        }
        return locations;
    }

    pub fn get_lead_suit(&self) -> Option<Suit> {
        self.current_trick[self.lead_player].map(|card| card.suit)
    }

    pub fn current_player_card_ids(&self) -> Vec<i32> {
        self.hands[self.current_player]
            .iter()
            .map(|c| c.id)
            .collect()
    }

    pub fn apply_move(&mut self, action: i32) {
        self.hide_playable();

        self.changes = vec![];
        if !self.get_moves().contains(&action) {
            panic!("Illegal move");
        }
        match self.state {
            State::BidSelectBidCard => self.apply_move_bid_card(action),
            State::BidSelectBidLocation => self.apply_move_bid_location(action),
            State::SelectCardToPlay => self.apply_move_select_card_to_play(action),
            State::SelectLocationToPlay => self.apply_move_select_location_to_play(action),
            State::SelectWinningOrLosing => self.apply_move_select_winning_or_losing(action),
        }

        // Show message for current action
        self.show_message();
        // Redraw playable cards for the current player
        self.show_playable();
    }

    pub fn apply_move_bid_card(&mut self, action: i32) {
        if action == PASS_BID {
            self.advance_player();
            return;
        }
        self.selected_card = Some(self.peek_card(action));
        self.state = State::BidSelectBidLocation;
    }

    pub fn apply_move_bid_location(&mut self, action: i32) {
        if action == UNDO {
            self.selected_card = None;
            self.state = State::BidSelectBidCard;
            return;
        }
        let card = self.pop_card(self.selected_card.unwrap().id);
        let offset = (action - BID_OFFSET) as usize;
        self.bids[offset] = Some(card.suit);
        // Animate bid card to position
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Bid,
                dest: Location::Bid,
                object_id: card.id,
                offset,
                ..Default::default()
            },
        );
        self.reorder_hand(self.current_player, false);
        if self.bids.iter().all(|x| x.is_some()) {
            self.state = State::SelectCardToPlay;
            self.current_player = self.dealer;
        } else {
            self.state = State::BidSelectBidCard;
            self.advance_player()
        }
    }

    fn apply_move_select_card_to_play(&mut self, action: i32) {
        if action == UNDO {
            if let Some(card) = self.current_trick[self.current_player].take() {
                self.hands[self.current_player].push(card);
                self.reorder_hand(self.current_player, true);
            }
            return;
        }
        if action == SKIP_MIX {
            // Animate the card that was previously played for CPU players
            let index = self.new_change();
            let card = self.current_trick[self.current_player].unwrap();
            self.add_change(
                index,
                Change {
                    change_type: ChangeType::Play,
                    object_id: card.id,
                    dest: Location::Play,
                    player: self.current_player,
                    ..Default::default()
                },
            );
            self.reorder_hand(self.current_player, false);
            self.advance_player();
            return;
        }
        self.selected_card = Some(self.peek_card(action));
        self.state = State::SelectLocationToPlay;
    }

    fn apply_move_select_location_to_play(&mut self, action: i32) {
        if action == UNDO {
            self.current_trick[self.current_player] = None;
            self.selected_card = None;
            self.state = State::SelectCardToPlay;
            return;
        }
        let left_card = self.pop_card(self.selected_card.unwrap().id);
        self.actual_trick_cards.push(left_card.clone());
        if self.trick_winning_player != self.current_player
            && action - PLAY_OFFSET == self.trick_winning_player as i32
        {
            // Smearing
            let right_card = self.current_trick[self.trick_winning_player].unwrap();
            let new_card = Card {
                id: self.next_id,
                suit: right_card.suit.mixed_with(left_card.suit),
                value: left_card.value + right_card.value,
            };
            self.next_id += 1;
            self.current_trick[self.trick_winning_player] = Some(new_card);
            // Check if existing played cards beat the new smeared card
            let start_winning_player = self.trick_winning_player;
            for i in 1..=3 {
                let index = (start_winning_player + i) % PLAYER_COUNT;
                if let Some(card) = self.current_trick[index] {
                    if card.beats(self.current_trick[self.trick_winning_player].unwrap()) {
                        self.trick_winning_player = index;
                    }
                }
            }
            self.animate_combine(self.trick_winning_player, new_card, left_card, right_card);
            self.state = State::SelectCardToPlay;
            self.reorder_hand(self.current_player, false);
            return;
        }

        let index = self.new_change();
        let mut card = left_card;

        if self.current_trick[self.current_player].is_some()
            && action - PLAY_OFFSET == self.current_player as i32
        {
            // Mixing
            let target_card = self.current_trick[self.current_player].unwrap();
            card = Card {
                id: self.next_id,
                suit: target_card.suit.mixed_with(card.suit),
                value: target_card.value + card.value,
            };
            self.next_id += 1;
            self.animate_combine(self.current_player, card, left_card, target_card);
        }

        if self.current_player == 0 {
            // Only reorder the human hand so they can see the partial play they made
            // For CPU players we want the mix animation to happen all at once
            self.reorder_hand(self.current_player, false);
        }

        self.current_trick[self.current_player] = Some(card);

        if self.trick_winning_player != self.current_player {
            // When a tie occurs the player has to select if they are winning or losing
            let target_card = self.current_trick[self.trick_winning_player].unwrap();
            if target_card.ties(card) {
                self.state = State::SelectWinningOrLosing;
                self.add_change(
                    index,
                    Change {
                        change_type: ChangeType::Play,
                        object_id: card.id,
                        dest: Location::Play,
                        player: self.current_player,
                        ..Default::default()
                    },
                );
                return;
            }
            if card.beats(target_card) {
                self.trick_winning_player = self.current_player;
            }
        }
        if !self.cards_playable_as_a_mix().is_empty() {
            self.state = State::SelectCardToPlay;
            return;
        }
        // No mixing is possible - animate the played card immediately
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Play,
                object_id: card.id,
                dest: Location::Play,
                player: self.current_player,
                ..Default::default()
            },
        );
        self.reorder_hand(self.current_player, false);
        self.state = State::SelectCardToPlay;
        self.advance_player();
    }

    fn apply_move_select_winning_or_losing(&mut self, action: i32) {
        match action {
            CHOOSE_TO_WIN => {
                self.trick_winning_player = self.current_player;
            }
            _ => {}
        }
        self.state = State::SelectCardToPlay;
        self.advance_player();
    }

    fn end_of_trick(&mut self) {
        // Show the winning card after the play animation of the last card finishes
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::ShowWinningCard,
                object_id: self.current_trick[self.trick_winning_player].unwrap().id,
                dest: Location::Play,
                ..Default::default()
            },
        );
        self.add_change(
            index,
            Change {
                change_type: ChangeType::OptionalPause,
                object_id: 0,
                dest: Location::Play,
                ..Default::default()
            },
        );
        self.lead_player = self.trick_winning_player;
        self.current_player = self.lead_player;
        let mut taken: Vec<Card> = self.actual_trick_cards.clone();
        let index = self.new_change();
        for card in taken.iter() {
            self.add_change(
                index,
                Change {
                    change_type: ChangeType::TricksToWinner,
                    dest: Location::TricksTaken,
                    player: self.trick_winning_player,
                    object_id: card.id,
                    ..Default::default()
                },
            );
        }
        // Discard spawned combined cards to make it clear that they will not score
        for card in self.current_trick.clone().iter().filter_map(|c| c.as_ref()) {
            if card.id < 100 {
                continue;
            }
            self.add_change(
                index,
                Change {
                    change_type: ChangeType::DeleteCard,
                    dest: Location::DeleteCard,
                    object_id: card.id,
                    ..Default::default()
                },
            );
        }
        self.cards_won[self.trick_winning_player].append(&mut taken);
        self.current_trick = [None; 4];
        self.actual_trick_cards = vec![];
        self.state = State::SelectCardToPlay;
        if self.hands[self.lead_player].is_empty() {
            self.end_of_hand();
        }
    }

    fn end_of_hand(&mut self) {
        self.set_suit_to_bid();
        for player in 0..PLAYER_COUNT {
            self.scores[player] += self.score_player(player);
        }
        self.cards_won = [vec![], vec![], vec![], vec![]];
        let max_score = self.scores.iter().max().unwrap();
        let min_score = self.scores.iter().min().unwrap();
        if *max_score >= POINT_THRESHOLD {
            // UI - emit game over
            for player in 0..PLAYER_COUNT {
                // Ties go to human player because they are 0
                if self.scores[player] == *min_score {
                    self.winner = Some(player);
                    break;
                }
            }
            return;
        }
        // Start a new hand if the game isn't over
        self.deal();
    }

    fn reduce_with_cancel(&self, cards: &Vec<Card>) -> Vec<Card> {
        let mut cancel_cards: Vec<Card> = vec![];
        let mut other_cards: Vec<Card> = vec![];

        for card in cards {
            let bid_space = self
                .suit_to_bid
                .get(&card.suit)
                .unwrap_or(&BidSpace::Missing);
            match *bid_space {
                BidSpace::Missing => {}
                BidSpace::Cancel => cancel_cards.push(*card),
                _ => other_cards.push(*card),
            }
        }

        // Sort descending by score
        other_cards.sort_by_key(|c| {
            let score = self
                .suit_to_bid
                .get(&c.suit)
                .unwrap_or(&BidSpace::Missing)
                .score_for_card(c);
            score
        });

        // Remove one card for each cancel
        for _ in 0..cancel_cards.len() {
            if other_cards.is_empty() {
                // No more cards on which to apply cancel cards
                break;
            }
            // Each cancel card removes the highest other card and is itself removed
            cancel_cards.pop();
            other_cards.pop();
        }

        // Add remaining cancel cards back
        other_cards.extend(cancel_cards);

        other_cards
    }

    pub fn score_player(&mut self, player: usize) -> i32 {
        let remaining_cards = self.reduce_with_cancel(&self.cards_won[player]);

        let mut score: i32 = 0;
        for card in remaining_cards.iter() {
            score += self
                .suit_to_bid
                .get(&card.suit)
                .unwrap_or(&BidSpace::Missing)
                .score_for_card(&card);
        }
        // UI - animate and emit scoring per suit group
        // TODO - automatically cancel highest point cards
        // and animate the cancel card away with those cards
        return score;
    }

    #[inline]
    fn new_change(&mut self) -> usize {
        self.changes.push(vec![]);
        self.changes.len() - 1
    }

    #[inline]
    fn advance_player(&mut self) {
        self.hide_playable();
        let cards_per_player = self.hands.iter().map(|h| h.len());
        if cards_per_player.filter(|c| *c > 0).count() <= 1 {
            self.end_of_hand();
            return;
        }
        let start_player = self.current_player;
        loop {
            self.current_player = (self.current_player + 1) % PLAYER_COUNT;
            if self.current_player == start_player {
                self.end_of_trick();
                break;
            }
            if self.hands[self.current_player].len() > 0
                && self.current_trick[self.current_player].is_none()
            {
                break;
            }
        }
    }

    #[inline]
    fn add_change(&mut self, index: usize, change: Change) {
        if self.no_changes {
            return;
        }
        self.changes[index].push(change);
    }

    #[inline]
    pub fn sort_hand(&mut self, player: usize) {
        if player != 0 {
            return;
        }
        self.hands[player].sort_by(human_card_sorter);
    }

    pub fn combine_sort(left_card: Card, right_card: Card) -> (Card, Card) {
        // Make the order of the cards match the text on the cards
        match (left_card.suit, right_card.suit) {
            (Suit::Yellow, Suit::Blue) => (right_card, left_card),
            (Suit::Blue, Suit::Red) => (right_card, left_card),
            (Suit::Red, Suit::Yellow) => (right_card, left_card),
            _ => (left_card, right_card),
        }
    }

    #[inline]
    pub fn animate_combine(
        &mut self,
        player_offset: usize,
        new_card: Card,
        left_card: Card,
        right_card: Card,
    ) {
        let (left_card, right_card) = PalaGame::combine_sort(left_card, right_card);
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Play,
                dest: Location::SpawnNewCard,
                object_id: new_card.id,
                card: Some(new_card),
                player: player_offset,
                offset: 0,
                length: 2,
                ..Default::default()
            },
        );
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Play,
                dest: Location::PlayCombine,
                object_id: left_card.id,
                player: player_offset,
                offset: 0,
                length: 2,
                ..Default::default()
            },
        );
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Play,
                dest: Location::PlayCombine,
                object_id: right_card.id,
                player: player_offset,
                offset: 1,
                length: 2,
                ..Default::default()
            },
        );
        self.reorder_hand(self.current_player, false);
    }

    #[inline]
    pub fn reorder_hand(&mut self, player: usize, force_new_animation: bool) {
        if self.no_changes {
            return;
        }
        if self.changes.is_empty() || force_new_animation {
            self.new_change();
        }
        let length = self.hands[player].len();
        let index = self.changes.len() - 1;
        self.changes[index].extend(self.hands[player].iter().enumerate().map(|(offset, card)| {
            Change {
                change_type: ChangeType::Reorder,
                dest: Location::ReorderHand,
                object_id: card.id,
                player,
                offset,
                length,
                ..Default::default()
            }
        }));
    }

    fn show_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.new_change();

        self.hide_playable();

        if Some(self.current_player) == self.human_player {
            let moves = self.get_moves();
            for id in moves {
                self.add_change(
                    change_index,
                    Change {
                        object_id: id,
                        change_type: ChangeType::ShowPlayable,
                        dest: Location::Hand,
                        player: self.current_player,
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn show_message(&mut self) {
        let player_name = self.player_name_string(self.current_player);
        let message = match self.state {
            State::BidSelectBidCard | State::BidSelectBidLocation => {
                Some(format!("{} may bid a card", player_name,))
            }
            State::SelectWinningOrLosing => Some(format!(
                "{} may select to be winning or losing",
                player_name,
            )),
            State::SelectLocationToPlay => {
                if Some(self.current_player) == self.human_player {
                    Some("Please select a location to play".to_string())
                } else {
                    Some("".to_string())
                }
            }
            _ => Some("".to_string()),
        };
        let index = self.new_change();
        self.set_message(message, index);
    }

    fn player_name_string(&self, player: usize) -> String {
        match player {
            0 => "You".to_string(),
            1 => "West".to_string(),
            2 => "North".to_string(),
            _ => "East".to_string(),
        }
    }

    fn set_message(&mut self, message: Option<String>, index: usize) {
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Message,
                message,
                object_id: -1,
                dest: Location::Message,
                ..Default::default()
            },
        );
    }

    fn hide_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.changes.len() - 1;
        let cards = self.hands[0].clone();
        for card in cards {
            self.add_change(
                change_index,
                Change {
                    object_id: card.id,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    player: self.current_player,
                    ..Default::default()
                },
            );
        }

        for location_based_move in LOCATION_BASED_MOVES {
            self.add_change(
                change_index,
                Change {
                    object_id: *location_based_move,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    player: self.current_player,
                    ..Default::default()
                },
            );
        }
    }
}

fn human_card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

impl ismcts::Game for PalaGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        // FIXME: implement determination for Pala
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.current_player
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.current_player + 1) % PLAYER_COUNT
    }

    fn available_moves(&self) -> Self::MoveList {
        self.get_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.apply_move(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.scores == [0; 4] {
            None
        } else {
            // High scores are bad
            // Worst score for a single hand appears to be 60
            // +FACE secondary (9 8 7 6 5 4 3 2) = 44
            // +1 (8) +1 (8) = 16

            let raw_score = self.scores[player] as f64;
            let normalized = raw_score / 60.0; // [0.0 (best), 1.0 (worst)]

            // Flip and apply exponential decay
            let shaped = 1.0 - normalized.powf(2.0); // You can tune the exponent (e.g., 1.5, 2.0)

            // Map to [-1.0, 1.0]
            return Some((shaped * 2.0) - 1.0);
        }
    }
}

pub fn get_mcts_move(game: &PalaGame, iterations: i32, debug: bool) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    // reset scores for the simulation
    new_game.scores = [0; 4];
    let mut ismcts = IsmctsHandler::new(new_game);
    let parallel_threads: usize = 8;
    ismcts.run_iterations(
        parallel_threads,
        (iterations as f64 / parallel_threads as f64) as usize,
    );
    ismcts.best_move().expect("should have a move to make")
}

#[cfg(test)]
mod tests {
    use super::*;
    use enum_iterator::all;
    use std::collections::HashMap;

    #[test]
    fn test_deck_composition() {
        let deck = PalaGame::deck();
        // Total card count
        assert_eq!(deck.len(), 48);

        // Group values by suit
        let mut map: HashMap<Suit, Vec<i32>> = HashMap::new();
        for card in deck {
            map.entry(card.suit).or_default().push(card.value);
        }

        for suit in all::<Suit>() {
            let values = map
                .get(&suit)
                .expect(&format!("No cards for suit {:?}", suit));
            // 8 cards per suit
            assert_eq!(values.len(), 8, "wrong count for {:?}", suit);

            // Sort and compare against expected multiset
            let mut got = values.clone();
            got.sort_unstable();
            let mut want = if suit.is_primary() {
                vec![1, 1, 2, 2, 3, 3, 4, 5]
            } else {
                vec![2, 3, 4, 5, 6, 7, 8, 9]
            };
            want.sort_unstable();
            assert_eq!(got, want, "bad values for {:?}", suit);
        }
    }
    #[derive(Debug)]
    struct ScoreScenario {
        name: String,
        cards_won: Vec<Card>,
        expected_score: i32,
    }

    #[test]
    fn test_scoring() {
        let mut game = PalaGame::new();

        game.bids = [
            // BidSpace::PlusFace,
            Some(Suit::Orange),
            // BidSpace::PlusOne,
            Some(Suit::Purple),
            // BidSpace::PlusOne,
            Some(Suit::Red),
            // BidSpace::Cancel,
            Some(Suit::Green),
        ];
        game.set_suit_to_bid();

        let scenarios = vec![
            ScoreScenario {
                name: "unbid suits should not score".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Blue,
                    value: 5,
                }],
                expected_score: 0,
            },
            ScoreScenario {
                name: "face suits score face value".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Orange,
                    value: 7,
                }],
                expected_score: 7,
            },
            ScoreScenario {
                name: "+1 suits score 1 point".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Red,
                    value: 7,
                }],
                expected_score: 1,
            },
            ScoreScenario {
                name: "cancel suits score -1 point".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Green,
                    value: 5,
                }],
                expected_score: -1,
            },
            ScoreScenario {
                name: "cancel removes highest scoring card".to_string(),
                cards_won: vec![
                    Card {
                        id: 0,
                        suit: Suit::Green,
                        value: 1,
                    }, // cancel
                    Card {
                        id: 1,
                        suit: Suit::Orange,
                        value: 9,
                    }, // +face
                    Card {
                        id: 2,
                        suit: Suit::Red,
                        value: 4,
                    }, // +1
                ],
                expected_score: 1, // Orange 9 is canceled, Red gives +1, cancel is consumed (0)
            },
            ScoreScenario {
                name: "cancel removes highest of multiple +face".to_string(),
                cards_won: vec![
                    Card {
                        id: 0,
                        suit: Suit::Green,
                        value: 1,
                    }, // cancel
                    Card {
                        id: 1,
                        suit: Suit::Orange,
                        value: 7,
                    }, // +face
                    Card {
                        id: 2,
                        suit: Suit::Orange,
                        value: 5,
                    }, // +face
                ],
                expected_score: 5, // 7 is canceled, 5 is scored
            },
            ScoreScenario {
                name: "unused cancel scores -1".to_string(),
                cards_won: vec![
                    Card {
                        id: 0,
                        suit: Suit::Green,
                        value: 1,
                    }, // cancel
                    Card {
                        id: 1,
                        suit: Suit::Blue,
                        value: 2,
                    }, // unbid
                ],
                expected_score: -1, // cancel is not used, unbid scores 0
            },
            ScoreScenario {
                name: "two cancels remove top two".to_string(),
                cards_won: vec![
                    Card {
                        id: 0,
                        suit: Suit::Green,
                        value: 1,
                    }, // cancel
                    Card {
                        id: 1,
                        suit: Suit::Green,
                        value: 2,
                    }, // cancel
                    Card {
                        id: 2,
                        suit: Suit::Orange,
                        value: 7,
                    }, // +face
                    Card {
                        id: 3,
                        suit: Suit::Orange,
                        value: 6,
                    }, // +face
                    Card {
                        id: 4,
                        suit: Suit::Red,
                        value: 3,
                    }, // +1
                ],
                expected_score: 1, // Orange 7 and 6 are canceled, Red gives +1, cancels are consumed
            },
            ScoreScenario {
                name: "more cancels than scoring cards".to_string(),
                cards_won: vec![
                    Card {
                        id: 0,
                        suit: Suit::Green,
                        value: 1,
                    }, // cancel
                    Card {
                        id: 1,
                        suit: Suit::Green,
                        value: 2,
                    }, // cancel
                    Card {
                        id: 2,
                        suit: Suit::Red,
                        value: 3,
                    }, // +1
                ],
                expected_score: -1, // One cancel removes Red, other cancel is unused
            },
        ];

        for scenario in scenarios {
            game.cards_won[0] = scenario.cards_won;
            assert_eq!(
                game.score_player(0),
                scenario.expected_score,
                "Scenario: {}, Cards: {:?} Expected score: {}",
                scenario.name,
                game.cards_won[0],
                scenario.expected_score
            );
        }
    }

    struct GetBidMovesScenario {
        name: String,
        bids: [Option<Suit>; PLAYER_COUNT],
        current_player: usize,
        hand: Vec<Card>,
        expected_moves_for_card_selection: Vec<i32>,
        card_selection_move: i32,
        expected_state_after_apply_move: State,
        expected_next_player: usize,
        expected_moves_after_card_selection: Option<Vec<i32>>,
        bid_offset_move: Option<i32>,
        expected_bids_after_bid_move: Option<[Option<Suit>; PLAYER_COUNT]>,
        expected_state_after_bid_move: Option<State>,
    }

    #[test]
    pub fn test_get_moves_bid_phase() {
        let red7 = Card {
            id: 0,
            suit: Suit::Red,
            value: 7,
        };

        let orange8 = Card {
            id: 1,
            suit: Suit::Orange,
            value: 8,
        };

        let purple5 = Card {
            id: 2,
            suit: Suit::Purple,
            value: 5,
        };

        let scenarios = [
            GetBidMovesScenario {
                name: "No bids yet - any suit can be bid".to_string(),
                bids: [None, None, None, None],
                current_player: 3,
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, orange8.id, purple5.id, PASS_BID],
                card_selection_move: orange8.id,
                expected_state_after_apply_move: State::BidSelectBidLocation,
                expected_next_player: 3,
                expected_moves_after_card_selection: Some(vec![
                    BID_OFFSET,
                    BID_OFFSET + 1,
                    BID_OFFSET + 2,
                    BID_OFFSET + 3,
                ]),
                bid_offset_move: Some(BID_OFFSET + 1),
                expected_bids_after_bid_move: Some([None, Some(Suit::Orange), None, None]),
                expected_state_after_bid_move: Some(State::BidSelectBidCard),
            },
            GetBidMovesScenario {
                name: "Cards matching previous bid not available to bid".to_string(),
                bids: [None, Some(Suit::Orange), None, None],
                current_player: 3,
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, purple5.id, PASS_BID],
                card_selection_move: red7.id,
                expected_state_after_apply_move: State::BidSelectBidLocation,
                expected_next_player: 3,
                expected_moves_after_card_selection: Some(vec![
                    BID_OFFSET,
                    BID_OFFSET + 2,
                    BID_OFFSET + 3,
                ]),
                bid_offset_move: Some(BID_OFFSET),
                expected_bids_after_bid_move: Some([
                    Some(Suit::Red),
                    Some(Suit::Orange),
                    None,
                    None,
                ]),
                expected_state_after_bid_move: Some(State::BidSelectBidCard),
            },
            GetBidMovesScenario {
                name: "Pass should move to the next player".to_string(),
                bids: [None, Some(Suit::Orange), None, None],
                current_player: 3,
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, purple5.id, PASS_BID],
                card_selection_move: PASS_BID,
                expected_state_after_apply_move: State::BidSelectBidCard,
                expected_next_player: 0,
                expected_moves_after_card_selection: None,
                bid_offset_move: None,
                expected_bids_after_bid_move: None,
                expected_state_after_bid_move: Some(State::BidSelectBidCard),
            },
            GetBidMovesScenario {
                name: "Should transition to play phase once the bid board is full".to_string(),
                bids: [
                    Some(Suit::Green),
                    Some(Suit::Orange),
                    Some(Suit::Yellow),
                    None,
                ],
                current_player: 3,
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, purple5.id, PASS_BID],
                card_selection_move: red7.id,
                expected_state_after_apply_move: State::BidSelectBidLocation,
                expected_next_player: 3,
                expected_moves_after_card_selection: Some(vec![BID_OFFSET + 3]),
                bid_offset_move: Some(BID_OFFSET + 3),
                expected_bids_after_bid_move: Some([
                    Some(Suit::Green),
                    Some(Suit::Orange),
                    Some(Suit::Yellow),
                    Some(Suit::Red),
                ]),
                expected_state_after_bid_move: Some(State::SelectCardToPlay),
            },
            GetBidMovesScenario {
                name: "Human player can undo bid card selection".to_string(),
                bids: [
                    Some(Suit::Green),
                    Some(Suit::Orange),
                    Some(Suit::Yellow),
                    None,
                ],
                current_player: 1,
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, purple5.id, PASS_BID],
                card_selection_move: red7.id,
                expected_state_after_apply_move: State::BidSelectBidLocation,
                expected_next_player: 1,
                expected_moves_after_card_selection: Some(vec![BID_OFFSET + 3, UNDO]),
                bid_offset_move: Some(UNDO),
                expected_bids_after_bid_move: Some([
                    Some(Suit::Green),
                    Some(Suit::Orange),
                    Some(Suit::Yellow),
                    None,
                ]),
                expected_state_after_bid_move: Some(State::BidSelectBidCard),
            },
        ];

        for scenario in scenarios {
            let mut game = PalaGame::new_with_human_player(1);
            game.current_player = scenario.current_player;
            game.state = State::BidSelectBidCard;
            game.hands[game.current_player] = scenario.hand;
            game.bids = scenario.bids;
            let moves = game.get_moves();
            assert_eq!(
                moves, scenario.expected_moves_for_card_selection,
                "Scenario: {}, Moves: {:?} Expected moves: {:?}",
                scenario.name, moves, scenario.expected_moves_for_card_selection
            );
            game.apply_move(scenario.card_selection_move);
            assert_eq!(
                game.current_player, scenario.expected_next_player,
                "Scenario: {}, Next player: {:?} Expected next player: {:?}",
                scenario.name, game.current_player, scenario.expected_next_player,
            );
            assert_eq!(
                game.state, scenario.expected_state_after_apply_move,
                "Scenario: {}, State: {:?} Expected state: {:?}",
                scenario.name, game.state, scenario.expected_state_after_apply_move,
            );
            let moves = game.get_moves();
            if scenario.expected_moves_after_card_selection.is_none() {
                continue;
            }
            let expected_moves = scenario.expected_moves_after_card_selection.unwrap();
            assert_eq!(
                moves, expected_moves,
                "Scenario: {}, Moves: {:?} Expected moves: {:?}",
                scenario.name, moves, expected_moves
            );
            let bid_offset_move = scenario.bid_offset_move.unwrap();
            game.apply_move(bid_offset_move);
            let expected_bids_after_bid_move = scenario.expected_bids_after_bid_move.unwrap();
            assert_eq!(
                game.bids, expected_bids_after_bid_move,
                "Scenario: {}, Bids: {:?} Expected bids: {:?}",
                scenario.name, game.bids, expected_bids_after_bid_move,
            );
            let expected_state_after_bid_move = scenario.expected_state_after_bid_move.unwrap();
            assert_eq!(
                game.state, expected_state_after_bid_move,
                "Scenario: {}, State: {:?} Expected state: {:?}",
                scenario.name, game.state, expected_state_after_bid_move,
            );
        }
    }

    struct PlayCardMoves {
        expected_moves_before: Vec<i32>,
        action: i32,
        expected_current_trick_after_move: [Option<Card>; PLAYER_COUNT],
        expected_state_after_move: State,
        expected_player_after_move: usize,
        expected_winning_player_after_move: usize,
    }

    struct PlayCardsScenario {
        name: String,
        current_trick: [Option<Card>; PLAYER_COUNT],
        trick_winning_player: usize,
        current_player: usize,
        lead_player: usize,
        hand: Vec<Card>,
        play_card_moves: Vec<PlayCardMoves>,
    }

    #[test]
    pub fn test_play_card_phase() {
        let red3 = Card {
            id: 0,
            suit: Suit::Red,
            value: 3,
        };

        let red7 = Card {
            id: 1,
            suit: Suit::Red,
            value: 7,
        };

        let orange8 = Card {
            id: 2,
            suit: Suit::Orange,
            value: 8,
        };

        let purple7 = Card {
            id: 4,
            suit: Suit::Purple,
            value: 7,
        };

        let purple8 = Card {
            id: 5,
            suit: Suit::Purple,
            value: 8,
        };

        let purple9 = Card {
            id: 6,
            suit: Suit::Purple,
            value: 9,
        };

        let blue5 = Card {
            id: 7,
            suit: Suit::Blue,
            value: 5,
        };

        let scenarios = [
            PlayCardsScenario {
                name: "Lead any card".to_string(),
                current_trick: [None, None, None, None],
                hand: vec![red7, orange8, purple8],
                current_player: 3,
                lead_player: 3,
                trick_winning_player: 3,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![red7.id, orange8.id, purple8.id],
                        action: red7.id,
                        expected_current_trick_after_move: [None, None, None, None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 3,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 3,
                        expected_current_trick_after_move: [None, None, None, Some(red7)],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 0,
                        expected_winning_player_after_move: 3,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Must follow".to_string(),
                current_trick: [None, None, Some(red7), None],
                hand: vec![red3, orange8, purple8],
                current_player: 3,
                lead_player: 2,
                trick_winning_player: 2,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![red3.id],
                        action: red3.id,
                        expected_current_trick_after_move: [None, None, Some(red7), None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 3,
                        expected_current_trick_after_move: [None, None, Some(red7), Some(red3)],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 0,
                        expected_winning_player_after_move: 2,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Can smear but chooses not to".to_string(),
                current_trick: [None, None, Some(red7), None],
                hand: vec![blue5, purple8],
                current_player: 3,
                lead_player: 2,
                trick_winning_player: 2,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![blue5.id, purple8.id],
                        action: blue5.id,
                        expected_current_trick_after_move: [None, None, Some(red7), None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 2, PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 3,
                        expected_current_trick_after_move: [None, None, Some(red7), Some(blue5)],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 0,
                        expected_winning_player_after_move: 2,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Can and does smear".to_string(),
                current_trick: [None, None, Some(red3), None],
                hand: vec![blue5, purple8],
                current_player: 3,
                lead_player: 2,
                trick_winning_player: 2,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![blue5.id, purple8.id],
                        action: blue5.id,
                        expected_current_trick_after_move: [None, None, Some(red3), None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 2, PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 2,
                        expected_current_trick_after_move: [
                            None,
                            None,
                            Some(Card {
                                id: 100,
                                value: 8,
                                suit: Suit::Purple,
                            }),
                            None,
                        ],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![purple8.id],
                        action: purple8.id,
                        expected_current_trick_after_move: [
                            None,
                            None,
                            Some(Card {
                                id: 100,
                                value: 8,
                                suit: Suit::Purple,
                            }),
                            None,
                        ],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 3,
                        expected_current_trick_after_move: [
                            None,
                            None,
                            Some(Card {
                                id: 100,
                                value: 8,
                                suit: Suit::Purple,
                            }),
                            Some(purple8),
                        ],
                        expected_state_after_move: State::SelectWinningOrLosing,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![CHOOSE_TO_WIN, CHOOSE_TO_LOSE],
                        action: CHOOSE_TO_WIN,
                        expected_current_trick_after_move: [
                            None,
                            None,
                            Some(Card {
                                id: 100,
                                value: 8,
                                suit: Suit::Purple,
                            }),
                            Some(purple8),
                        ],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 0,
                        expected_winning_player_after_move: 3,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Can and does smear - previously played card is higher".to_string(),
                current_trick: [None, Some(purple9), Some(red3), None],
                hand: vec![blue5, purple8],
                current_player: 3,
                lead_player: 2,
                trick_winning_player: 2,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![blue5.id, purple8.id],
                        action: blue5.id,
                        expected_current_trick_after_move: [None, Some(purple9), Some(red3), None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 2, PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 2,
                        expected_current_trick_after_move: [
                            None,
                            Some(purple9),
                            Some(Card {
                                id: 100,
                                value: 8,
                                suit: Suit::Purple,
                            }),
                            None,
                        ],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 1,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![purple8.id],
                        action: purple8.id,
                        expected_current_trick_after_move: [
                            None,
                            Some(purple9),
                            Some(Card {
                                id: 100,
                                value: 8,
                                suit: Suit::Purple,
                            }),
                            None,
                        ],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 1,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Can mix but chooses not to".to_string(),
                current_trick: [None, None, Some(purple8), None],
                hand: vec![blue5, red7],
                current_player: 3,
                lead_player: 2,
                trick_winning_player: 2,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![blue5.id, red7.id],
                        action: blue5.id,
                        expected_current_trick_after_move: [None, None, Some(purple8), None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 3,
                        expected_current_trick_after_move: [None, None, Some(purple8), Some(blue5)],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![red7.id, SKIP_MIX],
                        action: SKIP_MIX,
                        expected_current_trick_after_move: [None, None, Some(purple8), Some(blue5)],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 0,
                        expected_winning_player_after_move: 2,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Can and does mix".to_string(),
                current_trick: [None, None, Some(purple8), None],
                hand: vec![purple7, blue5, red7],
                current_player: 3,
                lead_player: 2,
                trick_winning_player: 2,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![purple7.id, blue5.id, red7.id],
                        action: blue5.id,
                        expected_current_trick_after_move: [None, None, Some(purple8), None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 3,
                        expected_current_trick_after_move: [None, None, Some(purple8), Some(blue5)],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![red7.id, SKIP_MIX],
                        action: red7.id,
                        expected_current_trick_after_move: [None, None, Some(purple8), Some(blue5)],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 3,
                        expected_winning_player_after_move: 2,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 3],
                        action: PLAY_OFFSET + 3,
                        expected_current_trick_after_move: [
                            None,
                            None,
                            Some(purple8),
                            Some(Card {
                                id: 100,
                                value: 12,
                                suit: Suit::Purple,
                            }),
                        ],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 0,
                        expected_winning_player_after_move: 3,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Human player undo smear".to_string(),
                current_trick: [Some(red3), None, None, None],
                hand: vec![blue5, purple8],
                current_player: 1,
                lead_player: 0,
                trick_winning_player: 0,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![blue5.id, purple8.id],
                        action: blue5.id,
                        expected_current_trick_after_move: [Some(red3), None, None, None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 1,
                        expected_winning_player_after_move: 0,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET, PLAY_OFFSET + 1, UNDO],
                        action: UNDO,
                        expected_current_trick_after_move: [Some(red3), None, None, None],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 1,
                        expected_winning_player_after_move: 0,
                    },
                ],
            },
            PlayCardsScenario {
                name: "Human player undo mix".to_string(),
                current_trick: [Some(purple8), None, None, None],
                hand: vec![blue5, red7],
                current_player: 1,
                lead_player: 0,
                trick_winning_player: 0,
                play_card_moves: vec![
                    PlayCardMoves {
                        expected_moves_before: vec![blue5.id, red7.id],
                        action: blue5.id,
                        expected_current_trick_after_move: [Some(purple8), None, None, None],
                        expected_state_after_move: State::SelectLocationToPlay,
                        expected_player_after_move: 1,
                        expected_winning_player_after_move: 0,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![PLAY_OFFSET + 1, UNDO],
                        action: PLAY_OFFSET + 1,
                        expected_current_trick_after_move: [Some(purple8), Some(blue5), None, None],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 1,
                        expected_winning_player_after_move: 0,
                    },
                    PlayCardMoves {
                        expected_moves_before: vec![red7.id, SKIP_MIX, UNDO],
                        action: UNDO,
                        expected_current_trick_after_move: [Some(purple8), None, None, None],
                        expected_state_after_move: State::SelectCardToPlay,
                        expected_player_after_move: 1,
                        expected_winning_player_after_move: 0,
                    },
                ],
            },
        ];

        for scenario in scenarios {
            let mut game = PalaGame::new_with_human_player(1);
            game.current_player = scenario.current_player;
            game.lead_player = scenario.lead_player;
            game.trick_winning_player = scenario.trick_winning_player;
            game.state = State::SelectCardToPlay;
            game.hands[game.current_player] = scenario.hand;
            game.current_trick = scenario.current_trick;
            for pcm in scenario.play_card_moves {
                let expected_moves = pcm.expected_moves_before;
                let actual_moves = game.get_moves();
                assert_eq!(
                    actual_moves, expected_moves,
                    "Scenario: {}, Actual moves: {:?} Expected moves: {:?}",
                    scenario.name, actual_moves, expected_moves,
                );
                game.apply_move(pcm.action);
                let actual_trick = game.current_trick;
                let expected_trick = pcm.expected_current_trick_after_move;
                assert_eq!(
                    actual_trick, expected_trick,
                    "Scenario: {}, Actual trick: {:?} Expected trick: {:?}",
                    scenario.name, actual_trick, expected_trick,
                );
                let actual_state = game.state.clone();
                let expected_state = pcm.expected_state_after_move;
                assert_eq!(
                    actual_state, expected_state,
                    "Scenario: {}, Actual state: {:?} Expected state: {:?}",
                    scenario.name, actual_state, expected_state,
                );
                let actual_player = game.current_player;
                let expected_player = pcm.expected_player_after_move;
                assert_eq!(
                    actual_player, expected_player,
                    "Scenario: {}, Actual player: {:?} Expected player: {:?}",
                    scenario.name, actual_player, expected_player,
                );
                let actual_winning_player = game.trick_winning_player;
                let expected_winning_player = pcm.expected_winning_player_after_move;
                assert_eq!(
                    actual_winning_player, expected_winning_player,
                    "Scenario: {}, Actual winning player: {:?} Expected winning player: {:?}",
                    scenario.name, actual_winning_player, expected_winning_player,
                );
            }
        }
    }

    struct CombineSortScenario {
        name: String,
        cards: [Card; 2],
        expected_order: [Card; 2],
    }

    #[test]
    fn test_combine_sort() {
        let red1 = Card {
            id: 0,
            suit: Suit::Red,
            value: 1,
        };
        let yellow1 = Card {
            id: 2,
            suit: Suit::Yellow,
            value: 2,
        };
        let blue1 = Card {
            id: 4,
            suit: Suit::Blue,
            value: 1,
        };
        let scenarios = [
            CombineSortScenario {
                name: "GREEN -> BLUE + YELLOW incorrect order".to_string(),
                cards: [yellow1, blue1],
                expected_order: [blue1, yellow1],
            },
            CombineSortScenario {
                name: "GREEN -> BLUE + YELLOW correct order".to_string(),
                cards: [blue1, yellow1],
                expected_order: [blue1, yellow1],
            },
            CombineSortScenario {
                name: "ORANGE -> YELLOW + RED incorrect order".to_string(),
                cards: [red1, yellow1],
                expected_order: [yellow1, red1],
            },
            CombineSortScenario {
                name: "ORANGE -> YELLOW + RED correct order".to_string(),
                cards: [yellow1, red1],
                expected_order: [yellow1, red1],
            },
            CombineSortScenario {
                name: "PURPLE -> RED + BLUE incorrect order".to_string(),
                cards: [blue1, red1],
                expected_order: [red1, blue1],
            },
            CombineSortScenario {
                name: "PURPLE -> RED + BLUE correct order".to_string(),
                cards: [red1, blue1],
                expected_order: [red1, blue1],
            },
        ];
        for scenario in scenarios {
            let new_cards = PalaGame::combine_sort(scenario.cards[0], scenario.cards[1]);
            let expected = (scenario.expected_order[0], scenario.expected_order[1]);
            let actual = new_cards;
            assert_eq!(
                actual, expected,
                "Scenario: {}, Actual player: {:?} Expected player: {:?}",
                scenario.name, actual, expected,
            );
        }
    }
}
