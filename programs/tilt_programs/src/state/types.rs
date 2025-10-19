use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum GameStage {
    WaitingForPlayers,
    WaitingForCommitments,
    WaitingForReveals,
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
    Completed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PlayerActionType {
    None,
    Fold,
    Check,
    Call,
    Raise,
    AllIn,
}

/// Card utilities
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Card(pub u8); // 0-51

impl Card {
    pub fn rank(self) -> u8 {
        self.0 % 13 // 0=2, 1=3, ..., 8=10, 9=J, 10=Q, 11=K, 12=A
    }

    pub fn suit(self) -> u8 {
        self.0 / 13 // 0=clubs, 1=diamonds, 2=hearts, 3=spades
    }

    pub fn rank_value(self) -> u8 {
        // Returns value for comparison (2=2, 3=3, ..., 10=10, J=11, Q=12, K=13, A=14)
        self.rank() + 2
    }
}

// /// Hand rankings (lower is better, like in poker)
//     HighCard = 0,
//     OnePair = 1,
//     TwoPair = 2,
//     ThreeOfAKind = 3,
//     Straight = 4,
//     Flush = 5,
//     FullHouse = 6,
//     FourOfAKind = 7,
//     StraightFlush = 8,
//     RoyalFlush = 9

