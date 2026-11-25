//! Ratings. The rules and formulas are PvPGN's `bnetd/ladder_calc.cpp`: Elo, with more than two sides treated as a
//! knockout tournament.

use crate::store::{Outcome, Record};

pub const INITIAL_RATING: i64 = 1000;

/// The chance that someone rated `a` beats someone rated `b`.
fn p(a: f64, b: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((b - a) / 400.0))
}

/// How much one game counts. `record` already includes it.
fn weight(record: &Record) -> f64 {
    if record.wins + record.losses + record.disconnects < 30 {
        50.0
    } else if record.rating < 2400 {
        30.0
    } else {
        20.0
    }
}

/// How much each player's rating changes, given their records (with this game counted) and results.
pub fn changes(players: &[(Record, Outcome)]) -> Vec<i64> {
    players
        .iter()
        .enumerate()
        .map(|(index, (record, outcome))| {
            let mut ratings = vec![record.rating as f64];
            let others = players.iter().enumerate().filter(|(other, _)| *other != index);
            ratings.extend(others.filter(|(_, (_, o))| o != outcome).map(|(_, (r, _))| r.rating as f64));
            let opponents = ratings.len() - 1;
            let Some(chance) = chance(&ratings) else { return 0 };
            let team = (players.len() - opponents) as f64;
            let change = match outcome {
                Outcome::Win => weight(record) * (1.0 - chance) / team,
                _ => -weight(record) * chance,
            };
            change as i64
        })
        .collect()
}

/// The chance that the first wins a tournament against the others.
fn chance(ratings: &[f64]) -> Option<f64> {
    Some(match *ratings {
        [a, b] => p(a, b),
        [a, b, c] => three(a, b, c) / 3.0,
        [a, b, c, d] => four(a, b, c, d) / 3.0,
        [a, b, c, d, e] => {
            (five(a, b, c, d, e) + five(a, c, d, e, b) + five(a, d, e, b, c) + five(a, e, b, c, d)) / 30.0
        }
        [a, b, c, d, e, f] => six(a, b, c, d, e, f) / 45.0,
        [a, b, c, d, e, f, g] => {
            // PvPGN divides by 45, which gives seven equal players a chance of 1 each, and has "ad + ad" in one
            // term where the pattern calls for "gd + fd".
            (seven(a, b, c, d, e, f, g)
                + seven(a, c, b, d, e, f, g)
                + seven(a, d, c, b, e, f, g)
                + seven(a, e, c, d, b, f, g)
                + seven(a, f, c, d, e, b, g)
                + seven(a, g, c, d, e, f, b))
                / 315.0
        }
        [a, b, c, d, e, f, g, h] => {
            (eight(a, c, d, e, f, g, h) * p(a, b)
                + eight(a, b, d, e, f, g, h) * p(a, c)
                + eight(a, b, c, e, f, g, h) * p(a, d)
                + eight(a, b, c, d, f, g, h) * p(a, e)
                + eight(a, b, c, d, e, g, h) * p(a, f)
                + eight(a, b, c, d, e, f, h) * p(a, g)
                + eight(a, b, c, d, e, f, g) * p(a, h))
                / 315.0
        }
        _ => return None,
    })
}

fn three(a: f64, b: f64, c: f64) -> f64 {
    2.0 * p(a, b) * p(a, c) + p(b, c) * p(a, b) + p(c, b) * p(a, c)
}

fn four(a: f64, b: f64, c: f64, d: f64) -> f64 {
    p(a, b) * p(a, c) * (p(c, d) + p(b, d))
        + p(a, c) * p(a, d) * (p(d, b) + p(c, b))
        + p(a, b) * p(a, d) * (p(d, c) + p(b, c))
}

fn five(a: f64, b: f64, c: f64, d: f64, e: f64) -> f64 {
    p(a, b)
        * (p(a, c) * three(c, d, e)
            + p(a, d) * three(d, e, c)
            + p(a, e) * three(e, c, d)
            + p(b, c) * three(a, d, e)
            + p(b, d) * three(a, c, e)
            + p(b, e) * three(a, c, d))
}

fn six(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> f64 {
    let one = |a, b, c, d, e, f| four(a, b, c, d) * (p(e, f) * p(a, e) + p(f, e) * p(a, f));
    let two = |a, b, c, d, e, f| {
        four(c, d, e, f) * p(a, b) * p(a, c)
            + four(d, c, e, f) * p(a, b) * p(a, d)
            + four(e, c, d, f) * p(a, b) * p(a, e)
            + four(f, c, d, e) * p(a, b) * p(a, f)
    };
    one(a, b, c, d, e, f)
        + one(a, b, c, e, d, f)
        + one(a, b, e, d, c, f)
        + one(a, e, c, d, b, f)
        + one(a, b, c, f, d, e)
        + one(a, b, f, d, c, e)
        + one(a, f, c, d, b, e)
        + one(a, e, f, b, c, d)
        + one(a, e, f, c, b, d)
        + one(a, e, f, d, b, c)
        + two(a, b, c, d, e, f)
        + two(a, c, b, d, e, f)
        + two(a, d, b, c, e, f)
        + two(a, e, b, c, d, f)
        + two(a, f, b, c, d, e)
}

fn seven(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64) -> f64 {
    seven_part(a, b, c, d, e, f, g)
        + seven_part(a, b, d, c, e, f, g)
        + seven_part(a, b, e, d, c, f, g)
        + seven_part(a, b, f, d, e, c, g)
        + seven_part(a, b, g, d, e, f, c)
}

fn seven_part(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64) -> f64 {
    // One of w beats two of x, y and z before meeting a.
    let path = |w, x, y, z| {
        p(a, w)
            * (p(w, x) * p(w, y) * (p(y, z) + p(x, z))
                + p(w, y) * p(w, z) * (p(z, x) + p(y, x))
                + p(w, x) * p(w, z) * (p(z, y) + p(x, y)))
    };
    let direct = |x, y, z| {
        p(a, x) * p(a, y) * (p(y, z) + p(x, z))
            + p(a, y) * p(a, z) * (p(z, x) + p(y, x))
            + p(a, x) * p(a, z) * (p(z, y) + p(x, y))
    };
    p(a, b)
        * ((p(a, c) + p(b, c)) * (path(d, e, f, g) + path(e, d, f, g) + path(f, e, d, g) + path(g, e, f, d))
            + p(b, c)
                * ((p(b, d) + p(c, d)) * direct(e, f, g)
                    + (p(b, e) + p(c, e)) * direct(d, f, g)
                    + (p(b, f) + p(c, f)) * direct(e, d, g)
                    + (p(b, g) + p(c, g)) * direct(e, f, d)))
}

fn eight(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64) -> f64 {
    let part = |b, c, d, e, f, g| {
        p(a, b)
            * (eight_end(a, d, e, f, g) * p(b, c)
                + eight_end(a, c, e, f, g) * p(b, d)
                + eight_end(a, c, d, f, g) * p(b, e)
                + eight_end(a, c, d, e, g) * p(b, f)
                + eight_end(a, c, d, e, f) * p(b, g))
    };
    part(b, c, d, e, f, g)
        + part(c, b, d, e, f, g)
        + part(d, b, c, e, f, g)
        + part(e, b, c, d, f, g)
        + part(f, b, c, d, e, g)
        + part(g, b, c, d, e, f)
}

fn eight_end(a: f64, b: f64, c: f64, d: f64, e: f64) -> f64 {
    let (ab, ac, ad, ae) = (p(a, b), p(a, c), p(a, d), p(a, e));
    let (bc, bd, be, cd, ce, de) = (p(b, c), p(b, d), p(b, e), p(c, d), p(c, e), p(d, e));
    let (cb, db, dc, eb, ec, ed) = (1.0 - bc, 1.0 - bd, 1.0 - cd, 1.0 - be, 1.0 - ce, 1.0 - de);
    (bc * de + be * dc) * ((ab - ad) * bd + ad)
        + (bd * ce + be * cd) * ((ab - ac) * bc + ac)
        + (cb * de + db * ce) * ((ac - ad) * cd + ad)
        + (cd * eb + cb * ed) * ((ac - ae) * ce + ae)
        + (dc * eb + db * ec) * ((ad - ae) * de + ae)
        + (bd * ec + bc * ed) * ((ae - ab) * eb + ab)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(rating: i64, wins: i64) -> Record {
        Record { rating, wins, ..Record::default() }
    }

    #[test]
    fn equals_have_equal_chances_and_they_add_up() {
        for count in 2..=8 {
            let chance = chance(&vec![1500.0; count]).unwrap();
            assert!((chance - 1.0 / count as f64).abs() < 1e-9, "{count} players: {chance}");
        }
        // With different ratings, everyone's chances of winning must still add up to one.
        for count in 2..=8 {
            let ratings: Vec<f64> = (0..count).map(|n| 1000.0 + 130.0 * n as f64).collect();
            let total: f64 = (0..count)
                .map(|first| {
                    let mut order = ratings.clone();
                    order.swap(0, first);
                    chance(&order).unwrap()
                })
                .sum();
            assert!((total - 1.0).abs() < 1e-9, "{count} players: {total}");
        }
    }

    #[test]
    fn matches_pvpgn() {
        // From PvPGN's own functions, for ratings 1000, 1130, 1260 and so on.
        let expected = [
            (2, 0.321183080205),
            (3, 0.114943640092),
            (4, 0.028862642439),
            (5, 0.008373561763),
            (6, 0.002247458423),
            (8, 0.000039828299),
        ];
        for (count, expected) in expected {
            let ratings: Vec<f64> = (0..count).map(|n| 1000.0 + 130.0 * n as f64).collect();
            assert!((chance(&ratings).unwrap() - expected).abs() < 1e-11, "{count} players");
        }
    }

    #[test]
    fn the_winner_gains_what_the_loser_loses() {
        let game = [(record(1000, 1), Outcome::Win), (record(1000, 1), Outcome::Loss)];
        assert_eq!(changes(&game), [25, -25]);
        let upset = [(record(1000, 1), Outcome::Win), (record(1400, 1), Outcome::Loss)];
        assert_eq!(changes(&upset), [45, -45]);
    }

    #[test]
    fn experience_and_teams_lower_the_stakes() {
        let experienced = [(record(1000, 30), Outcome::Win), (record(2400, 30), Outcome::Win)];
        assert_eq!(changes(&experienced), [0, 0], "nobody lost");
        let game = [(record(1000, 30), Outcome::Win), (record(1000, 30), Outcome::Loss)];
        assert_eq!(changes(&game), [15, -15]);
        let team = [
            (record(1000, 1), Outcome::Win),
            (record(1000, 1), Outcome::Win),
            (record(1000, 1), Outcome::Loss),
            (record(1000, 1), Outcome::Loss),
        ];
        assert_eq!(changes(&team), [16, 16, -16, -16]);
    }
}
