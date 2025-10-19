# zkPoker Solana Program Architecture

## 1. Overview

This document outlines the on-chain program architecture for zkPoker, a trustless, two-player Texas Hold'em game on Solana. The architecture is based on a "mental poker" protocol using commutative encryption to ensure card privacy and fairness without a trusted dealer.

## 2. Program Instructions

The program exposes a set of instructions that players call to advance the game state. These instructions are designed to be simple, with complex cryptographic operations happening on the client side whenever possible.

### `create_game()`
Initializes a new game state account. Called only by Player 1.
-   **Accounts**:
    -   `game_state`: The new account to be created (PDA).
    -   `player1`: The signer and game creator.
    -   `system_program`: Required for account creation.
-   **Args**:
    -   `player1_ephemeral_pubkey`: Player 1's public key for the commutative cipher.
    -   `deck_merkle_root`: The Merkle root of the 52-card, singly-encrypted, shuffled deck.
-   **Logic**:
    1.  Creates and initializes the `GameState` account.
    2.  Sets `player1`, their ephemeral public key, and the `deck_merkle_root`.
    3.  Transfers the buy-in from Player 1 to the pot/escrow.
    4.  Sets the game status to `WaitingForPlayer2`.

### `join_game()`
Called by Player 2 to join an existing game.
-   **Accounts**:
    -   `game_state`: The game account to join.
    -   `player2`: The signer and joining player.
-   **Args**:
    -   `player2_ephemeral_pubkey`: Player 2's public key.
    -   `encrypted_cards`: An array of the 9 doubly-encrypted cards needed for the hand.
    -   `singly_encrypted_cards`: The array of the 9 original singly-encrypted cards from Player 1's deck.
    -   `card_merkle_proofs`: An array of 9 Merkle proofs, one for each of the `singly_encrypted_cards`.
-   **Logic**:
    1.  Validates that the game is waiting for a player.
    2.  **Verification**: For each of the 9 cards, the program performs two checks:
        a.  It hashes the provided `singly_encrypted_card` and uses the corresponding `merkle_proof` to verify it against the `deck_merkle_root`. This proves the card came from Player 1's deck.
        b.  It re-encrypts the `singly_encrypted_card` with `player2_ephemeral_pubkey` and confirms it matches the provided `doubly_encrypted_card`. This proves Player 2 encrypted correctly.
        *(Note: This verification step is computationally intensive and will require a high CU limit.)*
    3.  Sets `player2` and their ephemeral public key.
    4.  Stores the now-verified 9 doubly-encrypted cards.
    5.  Transfers the buy-in from Player 2.
    6.  Posts blinds automatically.
    7.  Sets the game status to `PreFlopBetting` and sets the turn to the first player to act.

### `player_action()`
The primary instruction for all betting actions.
-   **Accounts**:
    -   `game_state`: The active game.
    -   `player`: The signer making the action.
-   **Args**:
    -   `action`: An enum representing the move (`Fold`, `Check`, `Call`, `Raise`).
    -   `amount`: The value of the raise, if applicable.
-   **Logic**:
    1.  Verifies it is the signer's turn to act.
    2.  Validates the action against the current game state (e.g., cannot `Check` a bet).
    3.  Updates player stacks and the pot.
    4.  If a player folds, the game moves to `Finished`, and the other player wins.
    5.  If the betting round concludes, updates the game state to await the next street (e.g., `AwaitingFlopReveal`).
    6.  Otherwise, switches the turn to the other player.

### `reveal_community_cards()`
A two-step instruction to reveal the flop, turn, or river.
-   **Accounts**:
    -   `game_state`: The active game.
    -   `player`: The signer.
-   **Args**:
    -   `decryption_shares`: The player's decrypted shares of the community card(s) for the current street.
    -   `plaintext_cards` (optional, Player 2 only): The final plaintext of the cards.
-   **Logic**:
    1.  **Called by Player 1**: Submits their decryption shares. The program stores them and sets a timeout for Player 2. Status becomes `AwaitingPlayer2Share`.
    2.  **Called by Player 2**: Submits their decryption shares. The program combines them with Player 1's shares to get the plaintext cards.
    3.  **Verification**: The program immediately re-encrypts the revealed plaintext cards with both players' public keys and verifies they match the on-chain encrypted versions.
    4.  If verified, the plaintext cards are stored, and the status moves to the next betting round (e.g., `PostFlopBetting`).

### `resolve_hand()`
A two-step instruction for the final showdown.
-   **Accounts**:
    -   `game_state`: The active game.
    -   `player`: The signer.
-   **Args**:
    -   `plaintext_pocket_cards`: The player's two hole cards.
-   **Logic**:
    1.  **Called by First Player**: The player reveals their two plaintext pocket cards. The program verifies them against the stored encrypted versions. If valid, stores the hand and sets a timeout for the other player.
    2.  **Called by Second Player**: The player reveals their two plaintext pocket cards. The program verifies them.
    3.  **Winner Determination**: With both hands verified, the program combines them with the community cards, runs the hand-ranking logic, and determines the winner.
    4.  The pot is dispersed to the winner, and the game status is set to `Finished`.

### `claim_on_timeout()`
A catch-all instruction to handle unresponsive players.
-   **Accounts**:
    -   `game_state`: The active game.
    -   `player`: The signer claiming the timeout.
-   **Logic**:
    1.  Checks the game's current timestamp against the action deadline.
    2.  If the deadline has passed, the signer is declared the winner.
    3.  The pot and the opponent's bond are transferred to the signer.
    4.  The game status is set to `Finished`.

## 4. Game State

The on-chain `GameState` account stores all the necessary information to represent a single hand of poker. It is designed to be minimal while ensuring the game is secure and verifiable. It transitions from the old model (seen in `game.rs`) by removing the plaintext deck and hands, and adding state for the cryptographic protocol.

*(Note: The `[u8; 32]` type assumes the use of a 256-bit prime for the commutative cipher, resulting in 32-byte keys and encrypted values.)*

### Core Game State & Players
-   `game_id`: A unique identifier for the game.
-   `player1`: The public key of the first player.
-   `player2`: The public key of the second player.
-   `stage`: The current phase of the game (e.g., `PreFlopBetting`, `AwaitingFlopReveal`).
-   `current_player`: Tracks whose turn it is to act (1 or 2).
-   `dealer_button`: Tracks the dealer position.
-   `winner`: `Option<Pubkey>` storing the winner of the hand.

### Cryptographic State
*(This section replaces the `player_commitment` and plaintext `deck` fields from the old model.)*
-   `player1_ephemeral_pubkey`: `[u8; 32]` - Player 1's public key for this hand's encryption.
-   `player2_ephemeral_pubkey`: `[u8; 32]` - Player 2's public key for this hand's encryption.
-   `deck_merkle_root`: `[u8; 32]` - The Merkle root of Player 1's singly-encrypted 52-card deck, used to verify Player 2's card selection.
-   `doubly_encrypted_cards`: `[[u8; 32]; 9]` - The 9 cards for the hand, doubly-encrypted with both players' keys. A canonical mapping is used:
    -   Indices 0-1: Player 1's pocket cards.
    -   Indices 2-3: Player 2's pocket cards.
    -   Indices 4-6: The flop.
    -   Index 7: The turn.
    -   Index 8: The river.
-   `player1_decryption_shares`: `[[u8; 32]; 3]` - A temporary holding spot for Player 1's decryption shares during the two-step community card reveal. Sized for the flop, which is the largest reveal.
-   `revealed_community_cards`: `[u8; 5]` - Stores the plaintext of community cards as they are successfully revealed and verified. Unrevealed cards are marked with a sentinel value.
-   `player1_revealed_hand`: `[u8; 2]` - Stores Player 1's plaintext hand, only populated and verified during the showdown.
-   `player2_revealed_hand`: `[u8; 2]` - Stores Player 2's plaintext hand, only populated and verified during the showdown.

### Betting & Financial State
-   `token_vault` & `vault_bump`: The pot/escrow account for the game's funds.
-   `stake_amount`: The buy-in amount for the game.
-   `pot`: The current total amount wagered.
-   `player1_stack` & `player2_stack`: Each player's remaining chips.
-   `player1_current_bet` & `player2_current_bet`: The amount each player has bet in the current round.
-   `player1_folded`, `player2_folded`, `player1_all_in`, `player2_all_in`: Boolean flags for player status.

### Timing & Liveness
-   `created_at`: Timestamp of game creation.
-   `last_action_at`: Timestamp of the last player action, used for timeouts.
-   `action_timeout`: The duration in seconds a player has to take their action.

## 3. Game Life-cycle

The game progresses through a series of defined states. A player's client can bundle instructions (e.g., a `player_action` that ends a betting round can be sent in the same transaction as the first `reveal_community_cards` call) to streamline the flow.

1.  **`WaitingForPlayer2`**: Game created by P1.
2.  **`PreFlopBetting`**: P2 joins, blinds posted. Betting occurs via `player_action`.
3.  **`AwaitingFlopReveal`**: Pre-flop betting ends. Awaiting `reveal_community_cards`.
4.  **`PostFlopBetting`**: Flop is revealed and verified. Betting occurs.
5.  **`AwaitingTurnReveal`**: Flop betting ends. Awaiting `reveal_community_cards`.
6.  **`PostTurnBetting`**: Turn is revealed and verified. Betting occurs.
7.  **`AwaitingRiverReveal`**: Turn betting ends. Awaiting `reveal_community_cards`.
8.  **`PostRiverBetting`**: River is revealed and verified. Final betting round.
9.  **`Showdown`**: Final betting ends. Awaiting `resolve_hand` calls.
10. **`Finished`**: Hand is resolved either by a fold, a completed showdown, or a timeout. The account is now ready to be closed.
