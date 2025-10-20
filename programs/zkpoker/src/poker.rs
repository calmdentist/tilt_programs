use crate::state::Card;

/// Evaluates a 5-card poker hand and returns a score
/// Higher score = better hand
/// Score format: hand_rank (4 bits) + tiebreakers (remaining bits)
pub fn evaluate_hand(cards: &[u8; 5]) -> u32 {
    let mut cards: Vec<Card> = cards.iter().map(|&c| Card(c)).collect();
    cards.sort_by_key(|c| c.rank_value());
    cards.reverse(); // Highest first

    let ranks: Vec<u8> = cards.iter().map(|c| c.rank()).collect();
    let suits: Vec<u8> = cards.iter().map(|c| c.suit()).collect();

    // Check flush
    let is_flush = suits.iter().all(|&s| s == suits[0]);

    // Check straight
    let is_straight = check_straight(&ranks);
    let is_wheel = ranks == vec![12, 3, 2, 1, 0]; // A-2-3-4-5

    // Count ranks
    let rank_counts = count_ranks(&ranks);

    // Determine hand rank
    if is_straight && is_flush {
        if ranks[0] == 12 && ranks[1] == 11 { // A-K-Q-J-10
            // Royal Flush
            return 9 << 20 | (14 << 16);
        }
        // Straight Flush
        let high_card = if is_wheel { 5 } else { cards[0].rank_value() };
        return 8 << 20 | (high_card as u32) << 16;
    }

    if let Some(four_rank) = rank_counts.iter().find(|(_, count)| *count == 4) {
        // Four of a Kind
        let kicker = rank_counts.iter().find(|(_, count)| *count == 1).unwrap().0;
        return 7 << 20 | ((four_rank.0 + 2) as u32) << 16 | ((kicker + 2) as u32) << 12;
    }

    let three = rank_counts.iter().find(|(_, count)| *count == 3);
    let pair = rank_counts.iter().find(|(_, count)| *count == 2);

    if three.is_some() && pair.is_some() {
        // Full House
        let three_rank = three.unwrap().0 + 2;
        let pair_rank = pair.unwrap().0 + 2;
        return 6 << 20 | (three_rank as u32) << 16 | (pair_rank as u32) << 12;
    }

    if is_flush {
        // Flush
        let mut score = 5 << 20;
        for (i, card) in cards.iter().enumerate() {
            score |= (card.rank_value() as u32) << (16 - i * 4);
        }
        return score;
    }

    if is_straight {
        // Straight
        let high_card = if is_wheel { 5 } else { cards[0].rank_value() };
        return 4 << 20 | (high_card as u32) << 16;
    }

    if three.is_some() {
        // Three of a Kind
        let three_rank = three.unwrap().0 + 2;
        let kickers: Vec<u8> = rank_counts
            .iter()
            .filter(|(_, count)| *count == 1)
            .map(|(rank, _)| *rank + 2)
            .collect();
        return 3 << 20 
            | (three_rank as u32) << 16 
            | (kickers[0] as u32) << 12 
            | (kickers[1] as u32) << 8;
    }

    let pairs: Vec<u8> = rank_counts
        .iter()
        .filter(|(_, count)| *count == 2)
        .map(|(rank, _)| *rank + 2)
        .collect();

    if pairs.len() == 2 {
        // Two Pair
        let high_pair = pairs.iter().max().unwrap();
        let low_pair = pairs.iter().min().unwrap();
        let kicker = rank_counts
            .iter()
            .find(|(_, count)| *count == 1)
            .unwrap().0 + 2;
        return 2 << 20 
            | (*high_pair as u32) << 16 
            | (*low_pair as u32) << 12 
            | (kicker as u32) << 8;
    }

    if pairs.len() == 1 {
        // One Pair
        let pair_rank = pairs[0];
        let kickers: Vec<u8> = rank_counts
            .iter()
            .filter(|(_, count)| *count == 1)
            .map(|(rank, _)| *rank + 2)
            .collect();
        return 1 << 20 
            | (pair_rank as u32) << 16 
            | (kickers[0] as u32) << 12 
            | (kickers[1] as u32) << 8
            | (kickers[2] as u32) << 4;
    }

    // High Card
    let mut score = 0 << 20;
    for (i, card) in cards.iter().enumerate() {
        score |= (card.rank_value() as u32) << (16 - i * 4);
    }
    score
}

/// Finds the best 5-card hand from 7 cards (2 hole + 5 community)
pub fn find_best_hand(hole_cards: &[u8; 2], community_cards: &[u8; 5]) -> ([u8; 5], u32) {
    let mut all_cards = Vec::with_capacity(7);
    all_cards.extend_from_slice(hole_cards);
    all_cards.extend_from_slice(community_cards);

    let mut best_hand = [0u8; 5];
    let mut best_score = 0u32;

    // Generate all 21 possible 5-card combinations from 7 cards
    for i in 0..7 {
        for j in (i + 1)..7 {
            for k in (j + 1)..7 {
                for l in (k + 1)..7 {
                    for m in (l + 1)..7 {
                        let hand = [
                            all_cards[i],
                            all_cards[j],
                            all_cards[k],
                            all_cards[l],
                            all_cards[m],
                        ];
                        let score = evaluate_hand(&hand);
                        if score > best_score {
                            best_score = score;
                            best_hand = hand;
                        }
                    }
                }
            }
        }
    }

    (best_hand, best_score)
}

fn check_straight(ranks: &[u8]) -> bool {
    // Check normal straight
    if ranks[0] == ranks[1] + 1
        && ranks[1] == ranks[2] + 1
        && ranks[2] == ranks[3] + 1
        && ranks[3] == ranks[4] + 1
    {
        return true;
    }

    // Check wheel (A-2-3-4-5)
    if ranks == &[12, 3, 2, 1, 0] {
        return true;
    }

    false
}

fn count_ranks(ranks: &[u8]) -> Vec<(u8, usize)> {
    let mut counts: Vec<(u8, usize)> = Vec::new();
    
    for &rank in ranks {
        if let Some(entry) = counts.iter_mut().find(|(r, _)| *r == rank) {
            entry.1 += 1;
        } else {
            counts.push((rank, 1));
        }
    }
    
    // Sort by count (descending), then by rank (descending)
    counts.sort_by(|a, b| {
        if a.1 != b.1 {
            b.1.cmp(&a.1)
        } else {
            b.0.cmp(&a.0)
        }
    });
    
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_royal_flush() {
        // A♠ K♠ Q♠ J♠ 10♠
        let hand = [51, 50, 49, 48, 47]; // All spades, high cards
        let score = evaluate_hand(&hand);
        assert_eq!(score >> 20, 9); // Royal flush
    }

    #[test]
    fn test_straight_flush() {
        // 9♠ 8♠ 7♠ 6♠ 5♠
        let hand = [46, 45, 44, 43, 42];
        let score = evaluate_hand(&hand);
        assert_eq!(score >> 20, 8); // Straight flush
    }

    #[test]
    fn test_four_of_a_kind() {
        // A♠ A♥ A♦ A♣ K♠
        let hand = [51, 38, 25, 12, 50];
        let score = evaluate_hand(&hand);
        assert_eq!(score >> 20, 7); // Four of a kind
    }

    #[test]
    fn test_full_house() {
        // A♠ A♥ A♦ K♣ K♠
        let hand = [51, 38, 25, 11, 50];
        let score = evaluate_hand(&hand);
        assert_eq!(score >> 20, 6); // Full house
    }
}

