# Optimistic zkpoker game-flow

## Game creation
1. Player 1 (p_1) creates game and submits ephemeral pubkey to contract (pubkey1), shuffles and encrypts deck offchain.

## Joining a game
2. Player 2 (p_2) joins game, receives shuffled/encrypted deck from p_1 offchain, validates this against their pubkey, and then shuffles/encrypts again.
   p_2 then submits ephemeral pubkey, 9 game cards (doubly-encrypted) to contract. 
   p_2 also singly-decrypts p_1's pocket cards (positions 0,2) and submits to contract.

## Pre-flop betting
3. p_1 now has *action_timeout* time to either:
    a. claim_timeout - by proving that p_2's singly decrypted versions of p_1's pocket cards are invalid
    b. place a pre-flop bet (assume p_1 is always small blind)
   p_1 also has to submit singly-decrypted p_2 pocket cards (positions 1,3) to contract

4. p_2 now has *action_timeout* time to either:
    a. claim_timeout - by proving that p_1's singly decrypted versions of p_2's pocket cards are invalid
    b. call/raise/fold - if raise, we continue the betting cycle
   the player who calls last (p_1 or p_2) has to also singly-decrypt the flop (positions 4,5,6) and submit on chain

## Flop
5. Pre-flop betting concludes (assume no fold). The player who's turn it is (p_t) has *action_timeout* time to:
    a. claim_timeout - by proving that the singly-decrypted flop is invalid
    b. submit doubly-decrypted flop (plaintext) and action (check or bet)

6. p_t (the other player) now has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's doubly-decrypted flop is invalid
    b. submit action (call, fold, or raise) - if raise, we continue the betting cycle
   the player who calls last must submit singly-decrypted turn card on chain

## Turn
7. Flop betting concludes (assume no fold). The player who's turn it is (p_t) has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's singly-decrypted turn card is invalid
    b. submit doubly-decrypted turn card (plaintext) and action (check or bet)

8. p_t (the other player) now has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's doubly-decrypted turn card is invalid
    b. submit action (call, fold, or raise) - if raise, we continue the betting cycle
   the player who calls last must submit singly-decrypted river card on chain

## River
9. Turn card betting concludes (assume no fold). The player who's turn it is (p_t) has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's singly-decrypted river card is invalid
    b. submit doubly-decrypted river card (plaintext) and action (check or bet)

10. p_t (the other player) now has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's doubly-decrypted river card is invalid
    b. submit action (call, fold, or raise) - if raise, we continue the betting cycle
   the player who calls last must submit doubly-decrypted river card on chain

## Showdown
11. River card betting concludes (assume no fold). Both players have *action_timeout* to:
    a. submit plaintext (doubly-decrypted) pocket cards

12. Settle dispute (if any). Both players have *action_timeout* to:
    a. do nothing
    b. claim_timeout - by proving that the other player's plaintext pocket cards are invalid

13. If no dispute, either player can call resolve, the contract will compute who has the winning hand and disperse
    pot accordingly.