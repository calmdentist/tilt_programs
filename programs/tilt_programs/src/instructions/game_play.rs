use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::*;

/// Deal initial cards (pocket cards)
pub fn deal_initial(ctx: Context<DealInitial>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    
    require!(
        game.stage == GameStage::PreFlop,
        PokerError::InvalidGameStage
    );
    
    require!(
        game.player1_hand == [0u8; 2],
        PokerError::CardsAlreadyDealt
    );
    
    // Deal 2 cards to each player
    game.player1_hand[0] = game.deal_card();
    game.player2_hand[0] = game.deal_card();
    game.player1_hand[1] = game.deal_card();
    game.player2_hand[1] = game.deal_card();
    
    Ok(())
}

#[derive(Accounts)]
pub struct DealInitial<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
}

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
            GameStage::PreFlop | GameStage::Flop | GameStage::Turn | GameStage::River
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
            game.stage = GameStage::Completed;
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
    pub game_state: Account<'info, GameState>,
    
    pub player: Signer<'info>,
}

/// Advance to next street (flop, turn, river) or showdown
pub fn advance_street(ctx: Context<AdvanceStreet>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    
    // Check if betting round is complete
    require!(
        game.is_betting_round_complete(),
        PokerError::BettingRoundNotComplete
    );
    
    // If someone folded, game is over
    if game.player1_folded || game.player2_folded {
        game.stage = GameStage::Completed;
        return Ok(());
    }
    
    // Reset current bets for new street
    game.player1_current_bet = 0;
    game.player2_current_bet = 0;
    
    // In heads-up, big blind acts first post-flop
    game.current_player = if game.dealer_button == 1 { 2 } else { 1 };
    
    match game.stage {
        GameStage::PreFlop => {
            // Deal flop (3 cards)
            game.community_cards[0] = game.deal_card();
            game.community_cards[1] = game.deal_card();
            game.community_cards[2] = game.deal_card();
            game.community_cards_dealt = 3;
            game.stage = GameStage::Flop;
        }
        
        GameStage::Flop => {
            // Deal turn (1 card)
            game.community_cards[3] = game.deal_card();
            game.community_cards_dealt = 4;
            game.stage = GameStage::Turn;
        }
        
        GameStage::Turn => {
            // Deal river (1 card)
            game.community_cards[4] = game.deal_card();
            game.community_cards_dealt = 5;
            game.stage = GameStage::River;
        }
        
        GameStage::River => {
            // Go to showdown
            game.stage = GameStage::Showdown;
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
    pub game_state: Account<'info, GameState>,
}

