use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::*;

/// Handle player actions (fold, check, call, raise)
pub fn player_action(
    ctx: Context<PlayerAction>,
    action: PlayerActionType,
    raise_amount: Option<u64>,
) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    let player = ctx.accounts.player.key();
    let clock = Clock::get()?;
    
    // Verify it's a valid betting stage
    require!(
        matches!(
            game.stage,
            GameStage::PreFlopBetting | GameStage::PostFlopBetting | 
            GameStage::PostTurnBetting | GameStage::PostRiverBetting
        ),
        PokerError::InvalidGameStage
    );
    
    // Verify it's the player's turn
    require!(game.is_player_turn(&player), PokerError::NotYourTurn);
    
    let is_player1 = player == game.player1;
    let current_bet = if is_player1 {
        game.player1_current_bet
    } else {
        game.player2_current_bet
    };
    
    let opponent_bet = if is_player1 {
        game.player2_current_bet
    } else {
        game.player1_current_bet
    };
    
    let player_stack = if is_player1 {
        game.player1_stack
    } else {
        game.player2_stack
    };
    
    let opponent_stack = if is_player1 {
        game.player2_stack
    } else {
        game.player1_stack
    };
    
    // Check if player has folded or is all-in
    if is_player1 {
        require!(!game.player1_folded, PokerError::CannotActAfterFold);
        require!(!game.player1_all_in, PokerError::CannotRaiseAllIn);
    } else {
        require!(!game.player2_folded, PokerError::CannotActAfterFold);
        require!(!game.player2_all_in, PokerError::CannotRaiseAllIn);
    }
    
    match action {
        PlayerActionType::Fold => {
            if is_player1 {
                game.player1_folded = true;
            } else {
                game.player2_folded = true;
            }
            game.stage = GameStage::Finished;
            game.winner = Some(game.get_other_player(&player));
        }
        
        PlayerActionType::Check => {
            // Can only check if bets are equal
            require!(
                current_bet == opponent_bet,
                PokerError::InvalidAction
            );
        }
        
        PlayerActionType::Call => {
            let call_amount = opponent_bet.saturating_sub(current_bet);
            
            // If player doesn't have enough to call, they go all-in
            if player_stack < call_amount {
                // Player goes all-in with whatever they have
                let all_in_amount = player_stack;
                
                if is_player1 {
                    game.player1_current_bet = current_bet.saturating_add(all_in_amount);
                    game.player1_stack = 0;
                    game.player1_all_in = true;
                } else {
                    game.player2_current_bet = current_bet.saturating_add(all_in_amount);
                    game.player2_stack = 0;
                    game.player2_all_in = true;
                }
                
                game.pot = game.pot.saturating_add(all_in_amount);
            } else {
                // Normal call
                if is_player1 {
                    game.player1_current_bet = opponent_bet;
                    game.player1_stack = player_stack.saturating_sub(call_amount);
                } else {
                    game.player2_current_bet = opponent_bet;
                    game.player2_stack = player_stack.saturating_sub(call_amount);
                }
                
                game.pot = game.pot.saturating_add(call_amount);
            }
        }
        
        PlayerActionType::Raise => {
            let raise_amt = raise_amount.ok_or(PokerError::InvalidBetAmount)?;
            let call_amount = opponent_bet.saturating_sub(current_bet);
            let total_new_bet = call_amount.saturating_add(raise_amt);
            
            // Check if player has enough to raise
            require!(
                player_stack >= total_new_bet,
                PokerError::InsufficientFunds
            );
            
            // Calculate minimum raise (must be at least the size of the previous raise)
            let min_raise = opponent_bet.saturating_sub(current_bet);
            require!(
                raise_amt >= min_raise || raise_amt == player_stack.saturating_sub(call_amount),
                PokerError::MinimumRaiseNotMet
            );
            
            let new_bet = current_bet.saturating_add(total_new_bet);
            
            // If the raise amount is more than opponent's stack, cap it
            // The opponent can only call up to their stack
            let effective_bet = if new_bet > opponent_bet.saturating_add(opponent_stack) {
                opponent_bet.saturating_add(opponent_stack)
            } else {
                new_bet
            };
            
            if is_player1 {
                game.player1_current_bet = effective_bet;
                game.player1_stack = player_stack.saturating_sub(total_new_bet);
            } else {
                game.player2_current_bet = effective_bet;
                game.player2_stack = player_stack.saturating_sub(total_new_bet);
            }
            
            game.pot = game.pot.saturating_add(total_new_bet);
        }
        
        PlayerActionType::AllIn => {
            // Player goes all-in with their entire remaining stack
            require!(player_stack > 0, PokerError::InsufficientFunds);
            
            let all_in_amount = player_stack;
            let new_total_bet = current_bet.saturating_add(all_in_amount);
            
            if is_player1 {
                game.player1_current_bet = new_total_bet;
                game.player1_stack = 0;
                game.player1_all_in = true;
            } else {
                game.player2_current_bet = new_total_bet;
                game.player2_stack = 0;
                game.player2_all_in = true;
            }
            
            game.pot = game.pot.saturating_add(all_in_amount);
        }
        
        PlayerActionType::None => {
            return Err(PokerError::InvalidAction.into());
        }
    }
    
    game.last_action = action;
    game.last_action_at = clock.unix_timestamp;
    
    // Switch current player
    game.current_player = if game.current_player == 1 { 2 } else { 1 };
    
    Ok(())
}

#[derive(Accounts)]
pub struct PlayerAction<'info> {
    #[account(mut)]
    pub game_state: Box<Account<'info, GameState>>,
    
    pub player: Signer<'info>,
}

/// Advance to next street or showdown after betting round completes
/// This transitions to card reveal stages
pub fn advance_street(ctx: Context<AdvanceStreet>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    
    // Check if betting round is complete
    require!(
        game.is_betting_round_complete(),
        PokerError::BettingRoundNotComplete
    );
    
    // If someone folded, game is over
    if game.player1_folded || game.player2_folded {
        game.stage = GameStage::Finished;
        return Ok(());
    }
    
    let clock = Clock::get()?;
    
    // Transition based on current stage
    match game.stage {
        GameStage::PreFlopBetting => {
            // Move to awaiting flop reveal
            game.stage = GameStage::AwaitingFlopReveal;
            game.last_action_at = clock.unix_timestamp;
        }
        
        GameStage::PostFlopBetting => {
            // Move to awaiting turn reveal
            game.stage = GameStage::AwaitingTurnReveal;
            game.last_action_at = clock.unix_timestamp;
        }
        
        GameStage::PostTurnBetting => {
            // Move to awaiting river reveal
            game.stage = GameStage::AwaitingRiverReveal;
            game.last_action_at = clock.unix_timestamp;
        }
        
        GameStage::PostRiverBetting => {
            // Move to showdown - players will reveal their pocket cards
            game.stage = GameStage::Showdown;
            game.last_action_at = clock.unix_timestamp;
        }
        
        _ => {
            return Err(PokerError::InvalidGameStage.into());
        }
    }
    
    Ok(())
}

#[derive(Accounts)]
pub struct AdvanceStreet<'info> {
    #[account(mut)]
    pub game_state: Box<Account<'info, GameState>>,
}

