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
-   **Logic**:
    1.  Creates and initializes the `GameState` account.
    2.  Sets `player1` and their ephemeral public key.
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
-   **Logic**:
    1.  Validates that the game is waiting for a player.
    2.  Sets `player2` and their ephemeral public key.
    3.  Stores the 9 doubly-encrypted cards.
    4.  Transfers the buy-in from Player 2.
    5.  Posts blinds automatically.
    6.  Sets the game status to `PreFlopBetting` and sets the turn to the first player to act.

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
