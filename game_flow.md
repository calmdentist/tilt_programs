# Optimistic zkPoker game-flow

## Game creation
1. Player 1 (p_1) creates a hand, posting a small blind and committing to their singly-encrypted and shuffled deck by submitting its merkle root on-chain.
2. Player 1 also submits a `ProveCorrectDeckCreation` zk-SNARK.
   - This proof, which verifies the deck contains 52 unique cards, is **verified on-chain immediately**. The transaction will fail if the proof is invalid, guaranteeing a fair deck before the hand can begin.

## Joining a game
3. Player 2 (p_2) joins the game. Off-chain, p_1 sends their encrypted deck to p_2.
4. p_2's client validates the deck against the on-chain merkle root.
5. p_2 then reshuffles and doubly-encrypts the deck, and submits the following on-chain:
   - The new doubly-encrypted deck.
   - A `ProveCorrectReshuffle` zk-SNARK, proving the new deck is a valid transformation of p_1's original deck.
   - Singly-decrypted versions of p_1's pocket cards (positions 0,2).
   - A `ProveCorrectDecryption` zk-SNARK for p_1's cards.
   - The big blind.
   *Note: All zk-SNARKs submitted in this step are stored for potential disputes and are NOT verified on-chain immediately.*

## Pre-flop betting
6. p_1 now has *action_timeout* time to either:
    a. claim_timeout - by triggering on-chain verification of p_2's `ProveCorrectReshuffle` or `ProveCorrectDecryption` zk-SNARKs if their client finds them invalid.
    b. place a pre-flop action (raise/call/fold, assume p_1 is always small blind)
   p_1 also has to submit singly-decrypted p_2 pocket cards (positions 1,3) to contract (if not a fold), and a `ProveCorrectDecryption` zk-SNARK.

4. p_2 now has *action_timeout* time to either:
    a. claim_timeout - by proving that p_1's singly decrypted versions of p_2's pocket cards are invalid
    b. call/raise/fold - if raise, we continue the betting cycle
   the player who calls last (p_1 or p_2) has to also singly-decrypt the flop (positions 4,5,6) and submit on chain with ProveCorrectDecryption zk-snark

## Flop
5. Pre-flop betting concludes (assume no fold). The player who's turn it is (p_t) has *action_timeout* time to:
    a. claim_timeout - by proving that the singly-decrypted flop is invalid
    b. submit doubly-decrypted flop (plaintext) and action (check or bet) with ProveCorrectDecryption zk-snark

6. p_t (the other player) now has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's doubly-decrypted flop is invalid
    b. submit action (call, fold, or raise) - if raise, we continue the betting cycle
   the player who calls last must submit singly-decrypted turn card on chain with ProveCorrectDecryption zk-snark

## Turn
7. Flop betting concludes (assume no fold). The player who's turn it is (p_t) has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's singly-decrypted turn card is invalid
    b. submit doubly-decrypted turn card (plaintext) and action (check or bet) with ProveCorrectDecryption zk-snark

8. p_t (the other player) now has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's doubly-decrypted turn card is invalid
    b. submit action (call, fold, or raise) - if raise, we continue the betting cycle
   the player who calls last must submit singly-decrypted river card on chain with ProveCorrectDecryption zk-snark

## River
9. Turn card betting concludes (assume no fold). The player who's turn it is (p_t) has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's singly-decrypted river card is invalid
    b. submit doubly-decrypted river card (plaintext) and action (check or bet) with ProveCorrectDecryption zk-snark

10. p_t (the other player) now has *action_timeout* time to:
    a. claim_timeout - by proving that the other player's doubly-decrypted river card is invalid
    b. submit action (call, fold, or raise) - if raise, we continue the betting cycle
   the player who calls last must submit doubly-decrypted river card on chain with ProveCorrectDecryption zk-snark

## Showdown
11. River card betting concludes (assume no fold). Both players have *action_timeout* to:
    a. submit plaintext (doubly-decrypted) pocket cards with a `ProveCorrectDecryption` zk-SNARK.

12. Settle dispute (if any). Both players have *action_timeout* to:
    a. do nothing
    b. claim_timeout - by proving that the other player's `ProveCorrectDecryption` zk-SNARK for their pocket cards is invalid.

13. If no dispute, either player can call `resolve`. The contract will use its on-chain hand evaluation logic to determine the winner from the revealed community cards and the verified pocket cards, then disperse the pot accordingly.

*Each game can have arbitrarily many hands. After a hand concludes, the process loops, reusing the Paillier public keys stored in the main Game account. If at any point one player goes all in, the logic is different: the player going all in submits singly-decrypted versions of all remaining cards with a `ProveCorrectDecryption` zk-snark for each, and if the other player calls, they must do the same, then we skip to showdown*

*zk-SNARKS are optimistically stored on chain and only verified during a `claim_timeout` (dispute) call, minimizing transaction costs. The one exception is the `ProveCorrectDeckCreation` snark, which is verified upon submission to guarantee foundational fairness.*