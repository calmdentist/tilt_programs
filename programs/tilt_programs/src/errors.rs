use anchor_lang::prelude::*;

#[error_code]
pub enum PokerError {
    #[msg("Invalid game stage for this action")]
    InvalidGameStage,
    
    #[msg("Not your turn to act")]
    NotYourTurn,
    
    #[msg("Invalid player action")]
    InvalidAction,
    
    #[msg("Invalid bet amount")]
    InvalidBetAmount,
    
    #[msg("Insufficient funds for bet")]
    InsufficientFunds,
    
    #[msg("Invalid commitment hash")]
    InvalidCommitment,
    
    #[msg("Secret does not match commitment")]
    SecretMismatch,
    
    #[msg("Both secrets must be revealed before dealing")]
    SecretsNotRevealed,
    
    #[msg("Game is full")]
    GameFull,
    
    #[msg("Cannot join your own game")]
    CannotJoinOwnGame,
    
    #[msg("Action timeout not reached")]
    TimeoutNotReached,
    
    #[msg("Invalid raise amount - must be at least the previous bet")]
    RaiseTooSmall,
    
    #[msg("All players must post blinds")]
    BlindsNotPosted,
    
    #[msg("Betting round not complete")]
    BettingRoundNotComplete,
    
    #[msg("Cards already dealt")]
    CardsAlreadyDealt,
    
    #[msg("Invalid number of community cards")]
    InvalidCommunityCards,
    
    #[msg("Player has already folded")]
    PlayerFolded,
    
    #[msg("Player is all-in")]
    PlayerAllIn,

    #[msg("Invalid commitment - must not be zero hash")]
    ZeroCommitment,

    #[msg("Game already has both players")]
    GameAlreadyFull,

    #[msg("Cannot perform action - player has folded")]
    CannotActAfterFold,

    #[msg("Cannot raise - player is all-in")]
    CannotRaiseAllIn,

    #[msg("Minimum raise not met")]
    MinimumRaiseNotMet,
}

