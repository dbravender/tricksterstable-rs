/*
Game: Yokai Septet (2-player variant)
Yokai Septet Designers: yio, Muneyuki Yokouchi (横内宗幸)
2-player variant designer: Sean Ross
BoardGameGeek: https://boardgamegeek.com/boardgame/251433/yokai-septet
*/

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    default,
};

use enum_iterator::{all, Sequence};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, PartialOrd, Ord, Clone, Copy, Sequence, Serialize, Deserialize, Hash, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum Suit {
    Green,
    Purple,
    Pink,
    Yellow,
    Black,
    Red,
    Blue,
}

pub fn suit_offset(suit: Suit) -> i32 {
    match suit {
        Suit::Green => 0,
        Suit::Purple => 1,
        Suit::Pink => 2,
        Suit::Yellow => 3,
        Suit::Black => 4,
        Suit::Red => 5,
        Suit::Blue => 6,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: i32,
    pub value: i32,
    pub suit: Suit,
}

pub fn deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    let mut id = 0;
    for suit in all::<Suit>() {
        for value in 1..=7 {
            deck.push(Card {
                id,
                value: value + suit_offset(suit),
                suit,
            });
            id += 1;
        }
    }
    deck.shuffle(&mut thread_rng());
    deck
}

// Map<int, Card> idToCard = {for (var card in deck(null)) card.id: card};

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    Faceup,
    Trump,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    CaptureSeven,
    GameOver,
    RevealCard,
    Discard,
    Reorder,
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    Deck,
    Hand,
    Trump,
    Play,
    Faceup,
    TricksTaken,
    SevensPile,
    Score,
    StrawTop,
    StrawBottom,
    Discard,
    ReorderHand,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    #[serde(rename(serialize = "id", deserialize = "id"))]
    object_id: i32,
    dest: Location,
    tricks_taken: i32,
    start_score: i32,
    end_score: i32,
    hand_offset: usize,
    player: usize,
    length: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Discard,
    PlayCard,
    Swap,
    SelectSeven,
}

pub struct Yokai2pGame {
    state: State,
    trump_card: Option<Card>,
    hands: [Vec<Card>; 2],
    changes: Vec<Vec<Change>>,
    current_trick: [Option<Card>; 2],
    tricks_taken: [i32; 2],
    lead_suit: Option<Suit>,
    scores: [i32; 2],
    overall_scores: [i32; 2],
    pub voids: [HashSet<Suit>; 2],
    captured_sevens: [Vec<Card>; 2],
    straw_bottom: [Vec<Option<Card>>; 2],
    straw_top: [Vec<Option<Card>>; 2],
    current_player: usize,
    winner: Option<usize>,
    overall_winner: Option<usize>,
    lead_player: usize,
    round: i32,
}

impl Yokai2pGame {
    pub fn deal(mut self) {
        self.lead_suit = None;
        self.round += 1;
        self.tricks_taken = [0, 0];
        self.hands = [vec![], vec![]];
        self.state = State::Discard;
        self.current_player = self.lead_player;
        self.lead_player = (self.lead_player + 1) % 2;
        self.captured_sevens = [vec![], vec![]];
        self.voids = [HashSet::new(), HashSet::new()];
        let deal_index = self.changes.len() as usize;
        self.changes.extend(vec![]); // deal
        let mut cards = deck();
        let trump_card = cards.pop().unwrap();
        self.changes[deal_index].push(Change {
            change_type: ChangeType::Trump,
            object_id: trump_card.id,
            dest: Location::Trump,
            ..Default::default()
        });
        self.straw_bottom = [vec![], vec![]];
        for y in 0..7 {
            for player in 0..2 as usize {
                let card = cards.pop().unwrap();
                self.changes[deal_index].push(Change {
                    change_type: ChangeType::Deal,
                    object_id: card.id,
                    dest: Location::StrawBottom,
                    player,
                    hand_offset: y,
                    length: 7,
                    ..Default::default()
                });
                self.straw_bottom[player].push(Some(card));
            }
        }
        // To avoid having to deal with moving 7s - okayed with Sean
        let removed_sevens: Vec<Card> = cards.iter().filter(|c| c.value == 7).cloned().collect();
        cards.retain(|c| c.value != 7);
        // End dealing with sevens
        self.straw_top = [vec![], vec![]];
        for y in 0..6 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.changes[deal_index].push(Change {
                    change_type: ChangeType::Deal,
                    object_id: card.id,
                    dest: Location::StrawTop,
                    player,
                    hand_offset: y,
                    length: 6,
                    ..Default::default()
                });
                self.straw_top[player].push(Some(card));
            }
        }
        // To avoid having to deal with moving 7s - okayed with Sean
        cards.extend(removed_sevens);
        cards.shuffle(&mut thread_rng());
        // End dealing with sevens
        self.hands = [vec![], vec![]];
        for y in 0..11 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.hands[player].push(card);
            }
        }
        self.hands[0].sort_by(card_sorter);
        for y in 0..11 {
            for player in 0..2 {
                let card = self.hands[player][y];
                self.changes[deal_index].push(Change {
                    change_type: ChangeType::Deal,
                    object_id: card.id,
                    dest: Location::Hand,
                    player,
                    hand_offset: y,
                    length: 11,
                    ..Default::default()
                });
            }
        }
        self.show_playable();
    }

    fn show_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let mut changes = vec![];
        let change_len = self.changes.len();
        if self.current_player == 0 {
            for id in self.get_moves() {
                changes.push(Change {
                    object_id: id,
                    change_type: ChangeType::ShowPlayable,
                    dest: Location::Hand,
                    player: self.current_player,
                    ..Default::default()
                });
            }
            self.changes[change_len - 1] = changes;
        } else {
            self.hide_playable();
        }
    }

    fn hide_playable(&mut self) {
        let change_len = self.changes.len();
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let mut changes = vec![];

        let mut cards = self.hands[0].clone();
        cards.extend(self.exposed_straw_bottoms(0));
        cards.extend(
            self.straw_top[0]
                .iter()
                .filter_map(|x| *x)
                .collect::<Vec<_>>(),
        );
        for card in cards {
            changes.push(Change {
                object_id: card.id,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                player: self.current_player,
                ..Default::default()
            });
        }

        self.changes[change_len - 1] = changes;
    }

    fn exposed_straw_bottoms(&self, player: usize) -> HashSet<Card> {
        let mut exposed_cards: HashSet<Card> = HashSet::new();
        for (i, card) in self.straw_bottom[player].iter().cloned().enumerate() {
            if card.is_none() {
                continue;
            }
            let mut left_open: bool = false;
            let mut right_open: bool = false;
            if i == 0 {
                left_open = true;
            } else {
                left_open = self.straw_top[player][i - 1].is_none();
            }
            if i == 6 {
                right_open = true;
            } else {
                right_open = self.straw_top[player][i].is_none();
            }
            if left_open && right_open {
                exposed_cards.insert(card.unwrap());
            }
        }
        return exposed_cards;
    }

    fn get_moves(&self) -> Vec<i32> {
        if self.state == State::Discard {
            return self.hands[self.current_player]
                .iter()
                .filter(|c| c.value != 7) // can't discard 7s
                .map(|c| c.id)
                .collect();
        }
        // must follow
        let mut playable_cards = self.visible_straw(self.current_player);
        playable_cards.extend(self.hands[self.current_player].clone());

        if let Some(lead_suit) = self.lead_suit {
            let moves: Vec<i32> = playable_cards
                .iter()
                .filter(|c| c.suit == lead_suit)
                .map(|c| c.id)
                .collect();
            if !moves.is_empty() {
                return moves;
            }
        }
        return playable_cards.iter().map(|c| c.id).collect();
    }

    fn visible_straw(&self, player: usize) -> Vec<Card> {
        let mut visible: Vec<Card> = self.straw_top[player].iter().filter_map(|x| *x).collect();
        visible.extend(self.exposed_straw_bottoms(player));
        return visible;
    }

    fn hidden_straw(&self, player: usize) -> HashSet<Card> {
        let exposed_straw_bottoms = self.exposed_straw_bottoms(player);
        return self.straw_bottom[player]
            .iter()
            .filter_map(|x| *x)
            .filter(|x| !exposed_straw_bottoms.contains(x))
            .collect();
    }
}

// state = State.playCard;

/*
@JsonSerializable()
class Game implements GameState<Move, Player> {


  List<Change> revealStrawBottoms(int player) {
    return exposedStrawBottoms(player)
        .map((c) =>
            Change(type: ChangeType.revealCard, dest: Location.hand, id: c.id))
        .toList();
  }

  List<Change> reorderHand(int player) {
    List<Change> changes = [];
    List<Card> hand = hands[currentPlayer!];
    hand.asMap().forEach((offsetInHand, card) {
      changes.add(Change(
          dest: Location.reorderHand,
          id: card.id,
          player: player,
          type: ChangeType.reorder,
          handOffset: offsetInHand,
          length: hand.length));
    });
    return changes;
  }

  @override
  Game cloneAndApplyMove(Move move, Node<Move, Player>? root) {
    if (!getMoves().contains(move)) {
      parent = null;
      d.log('game: $this move: $move legalMoves: ${getMoves()}');
    }
    var newGame = clone();
    // reset previous MCTS round winner
    newGame.winner = null;
    // reset previous MCTS round scores
    newGame.scores = {0: 0, 1: 0};
    newGame.changes = [[]]; // card from player to table
    newGame.currentTrick = Map.from(currentTrick);
    List<Card> currentHand = newGame.hands[currentPlayer!];
    newGame.hands[currentPlayer!] = currentHand;
    var card = idToCard[move]!;
    newGame.playedCards[newGame.currentPlayer!].add(card);
    if (newGame.state == State.discard) {
      currentHand.remove(card);
      newGame.changes.add([
        Change(id: card.id, type: ChangeType.discard, dest: Location.discard),
        ...newGame.reorderHand(currentPlayer!),
      ]);
      if (newGame.hands.every((h) => h.length == 10)) {
        newGame.state = State.playCard;
      }
      newGame.currentPlayer = (currentPlayer! + 1) % 2;
      showPlayable(this, newGame);
      return newGame;
    }
    List<Card?> strawBottom = newGame.strawBottom[currentPlayer!];
    List<Card?> strawTop = newGame.strawTop[currentPlayer!];
    if (strawBottom.contains(card)) {
      int index = strawBottom.indexOf(card);
      strawBottom[index] = null;
    } else if (strawTop.contains(card)) {
      int index = strawTop.indexOf(card);
      strawTop[index] = null;
    } else {
      currentHand.remove(card);
    }
    newGame.changes[0].addAll([
      Change(
          type: ChangeType.play,
          id: move,
          dest: Location.play,
          player: currentPlayer!),
      ...newGame.reorderHand(currentPlayer!),
    ]);
    newGame.currentTrick[currentPlayer!] = card;
    if (newGame.leadSuit != null && card.suit != newGame.leadSuit) {
      // Player has revealed a void
      newGame.voids[currentPlayer!][newGame.leadSuit!] = true;
    }
    newGame.leadSuit ??= card.suit;
    newGame.currentPlayer = (currentPlayer! + 1) % 2;
    hidePlayable(this, newGame);
    // end trick
    if (newGame.currentTrick.length == 2) {
      int trickWinner = getWinner(
          newGame.leadSuit!, newGame.trumpCard!, newGame.currentTrick);
      Card winningCard = newGame.currentTrick[trickWinner]!;
      newGame.tricksTaken[trickWinner] = newGame.tricksTaken[trickWinner]! + 1;
      // winner of the trick leads
      newGame.currentPlayer = trickWinner;
      newGame.changes.add([
        Change(
            type: ChangeType.showWinningCard,
            id: winningCard.id,
            dest: Location.play),
        Change(type: ChangeType.optionalPause, id: 0, dest: Location.play),
        ...newGame.revealStrawBottoms(0),
        ...newGame.revealStrawBottoms(1),
      ]);
      List<Change> sevenChanges = [];
      List<Change> trickChanges = [];
      newGame.changes.addAll(
        [
          sevenChanges, // sevens
          trickChanges, // trick to team
        ],
      );
      newGame.currentTrick.forEach((player, card) {
        if (card.value == 7) {
          newGame.capturedSevens[trickWinner].add(card);
          sevenChanges.add(Change(
              type: ChangeType.captureSeven,
              id: card.id,
              dest: Location.sevensPile,
              handOffset: newGame.capturedSevens[trickWinner].indexOf(card),
              player: trickWinner));
        } else {
          trickChanges.add(Change(
              type: ChangeType.tricksToWinner,
              id: card.id,
              dest: Location.tricksTaken,
              player: trickWinner,
              tricksTaken: newGame.tricksTaken[trickWinner]!));
        }
      });
      newGame.currentTrick = {};
      newGame.leadSuit = null;

      int? handWinningPlayer;

      // player with >= 4 sevens wins the round
      newGame.capturedSevens.asMap().forEach((player, c7s) {
        if (c7s.length >= 4) {
          handWinningPlayer = player;
        }
      });

      // player with >= 7 tricks loses

      if (handWinningPlayer == null) {
        List<List<Card>> overallHands = [
          newGame.hands[0] +
              newGame.strawBottom[0].whereType<Card>().toList() +
              newGame.strawTop[0].whereType<Card>().toList(),
          newGame.hands[1] +
              newGame.strawBottom[1].whereType<Card>().toList() +
              newGame.strawTop[1].whereType<Card>().toList()
        ];
        if (overallHands.every((x) => x.isEmpty)) {
          handWinningPlayer = newGame.currentPlayer;
        }
        newGame.tricksTaken.forEach((player, tricks) {
          if (tricks >= 13) {
            // the other player won
            handWinningPlayer = (player + 1) % 2;
          }
        });
        if (handWinningPlayer != null) {
          List<Card> sevens = [];
          for (var hand in newGame.hands) {
            sevens.addAll(hand.where((x) => x.value == 7));
          }
          for (var pile in newGame.strawTop) {
            sevens.addAll(pile.whereType<Card>().where((x) => x.value == 7));
          }
          for (var pile in newGame.strawBottom) {
            sevens.addAll(pile.whereType<Card>().where((x) => x.value == 7));
          }
          newGame.capturedSevens[handWinningPlayer!].addAll(sevens);
          for (var seven in sevens) {
            sevenChanges.add(Change(
                type: ChangeType.captureSeven,
                id: seven.id,
                dest: Location.sevensPile,
                handOffset:
                    newGame.capturedSevens[handWinningPlayer!].indexOf(seven),
                player: handWinningPlayer!));
          }
        }
      }

      if (handWinningPlayer != null) {
        var c7s = newGame.capturedSevens[handWinningPlayer!];
        newGame.scores[handWinningPlayer!] =
            newGame.scores[handWinningPlayer]! +
                scoreSevens(c7s, newGame.trumpCard!);
        newGame.overallScores[handWinningPlayer!] =
            newGame.overallScores[handWinningPlayer]! +
                scoreSevens(c7s, newGame.trumpCard!);
        newGame.changes.add([
          Change(
              id: 0,
              type: ChangeType.score,
              dest: Location.score,
              startScore: overallScores[handWinningPlayer]!,
              endScore: newGame.overallScores[handWinningPlayer]!,
              player: handWinningPlayer!),
          Change(type: ChangeType.optionalPause, id: 0, dest: Location.play),
        ]);

        int? gameWinner;

        if (handWinningPlayer != null) {
          newGame.winner = handWinningPlayer;
        }

        newGame.overallScores.forEach((player, score) {
          if (score >= 7) {
            gameWinner = player;
            newGame.winner = player;
          }
        });

        if (gameWinner != null) {
          newGame.overallWinner = gameWinner;
          newGame.winner = gameWinner;
          newGame.changes.add(
              [Change(type: ChangeType.gameOver, id: 0, dest: Location.deck)]);
          return newGame;
        } else {
          newGame.changes.add(
              [Change(type: ChangeType.shuffle, id: 0, dest: Location.deck)]);
          newGame.deal();
          return newGame;
        }
      }
    }
    showPlayable(this, newGame);
    return newGame;
  }

  @override
  GameState<Move, Player>? determine(GameState<Move, Player>? initialState) {
    // return clone();
    var game = clone();
    List<Card> remainingCards = [];
    List<Set<Card>> hiddenStrawBottoms = [{}, {}];

    game.hands.asMap().forEach((player, hand) {
      if (player != currentPlayer) {
        remainingCards.addAll(hand);
      }
      var exposedStrawBottoms = game.exposedStrawBottoms(player);
      hiddenStrawBottoms[player] =
          Set.from(strawBottom[player].whereType<Card>())
            ..removeAll(exposedStrawBottoms);

      remainingCards.addAll(hiddenStrawBottoms[player]);
    });

    remainingCards.shuffle(random);
    game.hands.asMap().forEach((player, hand) {
      var originalHandLength = game.hands[player].length;
      if (player != currentPlayer) {
        var pc = extractShortSuitedCards(remainingCards, game.voids[player]);
        game.hands[player] = [];
        pc.cards.shuffle(random);
        for (var i = 0; i < originalHandLength; i++) {
          var card = pc.cards.removeAt(0);
          game.hands[player].add(card);
        }
        remainingCards = pc.leftovers + pc.cards;
      }
      assert(originalHandLength == game.hands[player].length);
    });

    remainingCards.shuffle(random);
    game.strawBottom.asMap().forEach((player, List<Card?> strawBottom) {
      strawBottom.asMap().forEach((i, card) {
        if (card != null &&
            hiddenStrawBottoms[player].contains(strawBottom[i])) {
          strawBottom[i] = remainingCards.removeAt(0);
        }
      });
    });

    assert(remainingCards.isEmpty);
    return game;
  }


  factory Game.fromJson(Map<String, dynamic> json) => _$GameFromJson(json);
  @override
  Map<String, dynamic> toJson() => _$GameToJson(this);

  @override
  String toString() {
    return toJson().toString();
  }
}

Player getWinner(Suit leadSuit, Card trumpCard, Map<Player, Card> trick) {
  Map<Card, Player> cardsToPlayer = {};
  trick.forEach((player, card) {
    cardsToPlayer[card] = player;
  });
  List<Card> cards = List.from(trick.values);
  cards.sort((a, b) => valueForCard(leadSuit, trumpCard, b)
      .compareTo(valueForCard(leadSuit, trumpCard, a)));
  return cardsToPlayer[cards[0]]!;
}

int suitSort(Card card) {
  return (suitOffsets[card.suit]! * 100) + card.value;
}

int valueForCard(Suit leadSuit, Card trumpCard, Card card) {
  if (card.value == 1 && card.suit == Suit.green) {
    return 1000;
  }
  if (card.suit == leadSuit) {
    return card.value + 50;
  }
  if (card.suit == trumpCard.suit) {
    return card.value + 100;
  }
  return card.value;
}

*/

pub fn seven_value(suit: &Suit) -> i32 {
    match suit {
        Suit::Green => 0,
        Suit::Purple => 0,
        Suit::Pink => 1,
        Suit::Yellow => 1,
        Suit::Black => 1,
        Suit::Red => 2,
        Suit::Blue => 2,
    }
}

pub fn score_sevens(sevens: Vec<Card>, trump_card: Card) -> i32 {
    sevens
        .iter()
        .filter(|&card| card.suit != trump_card.suit)
        .map(|card| seven_value(&card.suit))
        .sum()
}

pub fn card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}
