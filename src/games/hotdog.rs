/*
Game: Hotdog
Designer: Sean Ross
BoardGameGeek: https://boardgamegeek.com/boardgame/365349/hotdog
*/

enum Bid {
    NoPicker,
    Ketchup,
    Mustard,
    TheWorks,
    KetchupFootlong,
    MustardFootlong,
    TheWorksFootlong,
}

enum Ranking {
    HighStrong,
    LowStrong,
    Alternating,
}

impl Bid {
    fn requiredTricks(&self) -> i32 {
        match self {
            Bid::NoPicker => 9,
            Bid::Ketchup | Bid::Mustard | Bid::TheWorks => 9,
            Bid::KetchupFootlong | Bid::MustardFootlong | Bid::TheWorksFootlong => 12,
        }
    }

    fn ranking(&self) -> Ranking {
        match self {
            Bid::NoPicker => Ranking::Alternating,
            Bid::Ketchup | Bid::KetchupFootlong => Ranking::HighStrong,
            Bid::Mustard | Bid::MustardFootlong => Ranking::LowStrong,
            Bid::TheWorks | Bid::TheWorksFootlong => Ranking::Alternating,
        }
    }

    fn order(&self) -> i32 {
        match self {
            Bid::NoPicker => -1,
            Bid::Ketchup => 0,
            Bid::Mustard => 1,
            Bid::TheWorks => 2,
            Bid::KetchupFootlong => 3,
            Bid::MustardFootlong => 4,
            Bid::TheWorksFootlong => 5,
        }
    }

    fn points_for_setter(&self, tricks_taken: i32) -> i32 {
        match self {
            // Footlong Option
            // However, if the Picker fails to capture at least 12 tricks, the opponent automatically wins the game.
            Bid::KetchupFootlong | Bid::MustardFootlong | Bid::TheWorksFootlong => 5,
            _ => {
                if tricks_taken >= 12 {
                    5
                } else {
                    2
                }
            }
        }
    }

    fn points_for_picker_success(&self, tricks_taken: i32) -> i32 {
        match self {
            Bid::KetchupFootlong | Bid::MustardFootlong | Bid::TheWorksFootlong => {
                if tricks_taken >= 15 {
                    5
                } else {
                    3
                }
            }
            _ => {
                if tricks_taken >= 15 {
                    5
                } else if tricks_taken >= 12 {
                    2
                } else {
                    1
                }
            }
        }
    }
}

struct HotdogGame {}

impl HotdogGame {
    fn start_hand() {
        // The Picker leads to the first trick.
        // If both players pass, the non-dealer leads the first trick.
    }

    fn trick_winner() {
        // In general, the highest-ranking card in the trump suit wins the trick
        // or, if no trumps were played, the highest-ranking card in the suit that was led. However, if the trick includes two different suits, and one of the cards has the special rank, the card with the special rank wins.
        // If two special rank cards are played, the second card wins.
    }
}
