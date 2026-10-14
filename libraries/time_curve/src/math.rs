use crate::num::Num;

const DAY_SEC: u64 = 86400;
const YEAR_SEC: u64 = DAY_SEC * 365;

/// Get the proportion of PT in the market
///
/// Callers must pass the market's total PT and total asset reserves: the ratio
/// `pt / (pt + asset)` is only meaningful as the market's PT share when both
/// arguments describe the same market state, and callers of this crate rely on
/// that invariant (e.g. `trade` computes `p` from the pre-trade totals).
pub fn proportion_pt<N: Num>(pt: u64, asset: u64) -> N {
    let pt = N::from_u64(pt);
    let asset = N::from_u64(asset);

    pt / (pt + asset)
}

/// Logit function
pub fn logit<N: Num>(p: N) -> N {
    (p / (N::one() - p)).ln()
}

pub fn exchange_rate<N: Num>(logit: N, rate_scalar: N, rate_anchor: N) -> N {
    logit / rate_scalar + rate_anchor
}

/// Calculate the fee rate for a given time remaining and fee rate root
pub fn fee_rate<N: Num>(ln_fee_rate_root: N, sec_remaining: u64) -> N {
    let normalized_sec_remaining = normalized_sec_remaining::<N>(sec_remaining);
    let ln_fee_rate = ln_fee_rate_root * normalized_sec_remaining;
    ln_fee_rate.exp()
}

pub struct MarketProportions {
    pub pt: u64,
    pub sy: u64,
}

/// Calculate the proportional shares of PT and SY for a quantity of LP tokens
pub fn lp_proportion<N: Num>(
    lp_tokens: u64,
    market_pt: u64,
    market_sy: u64,
    total_lp: u64,
) -> MarketProportions {
    assert!(total_lp > 0);

    let lp_tokens = N::from_u64(lp_tokens);
    let market_pt = N::from_u64(market_pt);
    let market_sy = N::from_u64(market_sy);
    let total_lp = N::from_u64(total_lp);

    let pt = market_pt * lp_tokens / total_lp;
    let sy = market_sy * lp_tokens / total_lp;

    MarketProportions {
        pt: pt.to_u64(),
        sy: sy.to_u64(),
    }
}

/// Find the initial scalar root
/// It will be small for high divergence between rate_max and rate_expected
/// And it will be small if the tail is wide
pub fn rate_scalar_root<N: Num>(rate_expected: N, rate_max: N, tail_width: Option<N>) -> N {
    // "tail width" defines the range of normal trading proportions
    // the expected trading proportions should be between [tail_width, 1 - tail_width]
    let tail_width = tail_width.unwrap_or(N::from_ratio(1, 10));

    // the tail width cannot be zero, nor greater than 1/2
    assert!(tail_width > N::zero() && tail_width < N::from_ratio(1, 2));

    // The rate bounds bound the curve: rate_max must be above the expected rate,
    // and the expected rate must be above 1 (otherwise the denominators below are
    // zero and silently yield an infinite scalar root for the f64 backend).
    assert!(
        rate_max > rate_expected,
        "rate_max must be greater than rate_expected"
    );
    assert!(
        rate_expected > N::one(),
        "rate_expected must be greater than 1"
    );

    // small values of tail width will result in a large scalar root
    let s = ((N::one() - tail_width) / tail_width).ln();

    // low rate
    let a = s / (rate_max - rate_expected);
    // high rate
    let b = s / (rate_expected - N::one());

    a.min(b)
}

/// Normalize the seconds remaining by year, which is useful when computing the implied yield rate
pub fn normalized_sec_remaining<N: Num>(sec_remaining: u64) -> N {
    N::from_ratio(sec_remaining, YEAR_SEC)
}

/// The rate scalar grows as the seconds remaining decrease
///
/// The sensitivity of the curve has an inverse relationship to the rate scalar
/// As the rate scalar grows, the sensitivity goes down
pub fn rate_scalar<N: Num>(rate_scalar_root: N, sec_remaining: u64) -> N {
    // if sec_remaining is zero, the rate scalar is effectively infinite
    if sec_remaining == 0 {
        return N::max();
    }

    let normalized_sec_remaining = normalized_sec_remaining::<N>(sec_remaining);

    // root * YEAR_SEC / sec_remaining
    rate_scalar_root / normalized_sec_remaining
}

/// Natural log fo the implied interest rate
pub fn ln_implied_rate<N: Num>(
    pt: u64,
    asset: u64,
    rate_scalar: N,
    rate_anchor: N,
    sec_remaining: u64,
) -> N {
    // At the expiration boundary the normalized time is zero, and dividing by it
    // would produce inf/NaN for the f64 backend (or a panic for DNum). Mirror
    // `rate_scalar` and treat the implied rate as unbounded in that case.
    if sec_remaining == 0 {
        return N::max();
    }

    let l_p = logit(proportion_pt::<N>(pt, asset));
    let rate = exchange_rate::<N>(l_p, rate_scalar, rate_anchor);

    // ln() of a non-positive rate is undefined and evaluates to NaN silently on
    // the f64 backend, so reject it explicitly.
    assert!(
        rate > N::zero(),
        "exchange rate must be positive to compute ln_implied_rate"
    );

    let ln_rate = rate.ln();
    let normalized_sec_remaining = normalized_sec_remaining::<N>(sec_remaining);

    ln_rate / normalized_sec_remaining
}

/// Exchange rate is e^rt
pub fn exchange_rate_from_ln_implied_rate<N: Num>(ln_implied_rate: N, sec_remaining: u64) -> N {
    let rt = ln_implied_rate * normalized_sec_remaining::<N>(sec_remaining);

    rt.exp()
}

pub struct AddLiquidityResult {
    pub lp_tokens_out: u64,
    pub sy_in: u64,
    pub pt_in: u64,
}

/// Calculate the amount of LP tokens received, and SY & PT put in
/// This is based on an intended amount of SY and PT
pub fn add_liquidity<N: Num>(
    intent_sy: u64,
    intent_pt: u64,
    market_total_lp: u64,
    market_total_sy: u64,
    market_total_pt: u64,
) -> AddLiquidityResult {
    // A market with zero reserves or zero LP supply has no meaningful share
    // price: the ratios below divide by zero, which is inf for the f64 backend
    // (and `to_u64` then saturates to u64::MAX, minting an unbounded LP supply).
    // Reject a degenerate market instead of producing a nonsense result.
    assert!(
        market_total_lp > 0 && market_total_sy > 0 && market_total_pt > 0,
        "market must have non-zero LP, SY and PT totals to add liquidity"
    );

    let intent_sy = N::from_u64(intent_sy);
    let intent_pt = N::from_u64(intent_pt);
    let market_total_lp = N::from_u64(market_total_lp);
    let market_total_sy = N::from_u64(market_total_sy);
    let market_total_pt = N::from_u64(market_total_pt);

    let lp_from_pt = market_total_lp * intent_pt / market_total_pt;
    let lp_from_sy = market_total_lp * intent_sy / market_total_sy;

    if lp_from_pt < lp_from_sy {
        let lp_tokens_out = lp_from_pt;

        // When one intent leg is zero, no LP tokens are owed. The ceil formulas
        // below would still charge 1 unit of the other leg and mint nothing,
        // silently donating value to the pool. Return an empty result instead.
        if lp_tokens_out == N::zero() {
            return AddLiquidityResult {
                lp_tokens_out: 0,
                sy_in: 0,
                pt_in: 0,
            };
        }

        let pt_in = intent_pt.to_u64();
        let sy_in = ((market_total_sy * lp_tokens_out + market_total_lp - N::one())
            / market_total_lp)
            .to_u64();

        AddLiquidityResult {
            lp_tokens_out: lp_tokens_out.to_u64(),
            sy_in,
            pt_in,
        }
    } else {
        let lp_tokens_out = lp_from_sy;

        if lp_tokens_out == N::zero() {
            return AddLiquidityResult {
                lp_tokens_out: 0,
                sy_in: 0,
                pt_in: 0,
            };
        }

        let sy_in = intent_sy.to_u64();
        let pt_in = ((market_total_pt * lp_tokens_out + market_total_lp - N::one())
            / market_total_lp)
            .to_u64();

        AddLiquidityResult {
            lp_tokens_out: lp_tokens_out.to_u64(),
            sy_in,
            pt_in,
        }
    }
}

pub struct RemoveLiquidityResult {
    pub sy_out: u64,
    pub pt_out: u64,
}

/// Remove liquidity from the pool, returning the amount of SY and PT received for LP tokens in
pub fn rm_liquidity<N: Num>(
    lp_in: u64,
    market_total_lp: u64,
    market_total_sy: u64,
    market_total_pt: u64,
) -> RemoveLiquidityResult {
    assert!(market_total_lp >= lp_in);

    // The share math divides by the market's LP total, so it must be non-zero.
    assert!(market_total_lp > 0, "market LP total must be non-zero");

    let lp_in = N::from_u64(lp_in);
    let market_total_lp = N::from_u64(market_total_lp);
    let market_total_sy = N::from_u64(market_total_sy);
    let market_total_pt = N::from_u64(market_total_pt);

    let sy_out = market_total_sy * lp_in / market_total_lp;
    let pt_out = market_total_pt * lp_in / market_total_lp;

    RemoveLiquidityResult {
        sy_out: sy_out.to_u64(),
        pt_out: pt_out.to_u64(),
    }
}

/// Calculate amount of SY owned by an amount of LP tokens
pub fn lp_to_sy<N: Num>(
    lp_amount: u64,
    market_total_lp: u64,
    market_total_sy: u64,
    market_total_pt: u64,
) -> u64 {
    let r = rm_liquidity::<N>(lp_amount, market_total_lp, market_total_sy, market_total_pt);

    r.sy_out
}

/// Find a rate anchor that preserves the implied rate even though the exchange rate changes with time
pub fn find_rate_anchor<N: Num>(
    pt: u64,
    asset: u64,
    rate_scalar: N,
    last_ln_implied_rate: N,
    sec_remaining: u64,
) -> N {
    let new_exchange_rate =
        exchange_rate_from_ln_implied_rate::<N>(last_ln_implied_rate, sec_remaining);

    let p = logit(proportion_pt::<N>(pt, asset));

    // solve for rate anchor, which is a vertical translation on the curve
    new_exchange_rate - p / rate_scalar
}

/// Calculate the amount of asset fee on a trade
/// If net_trader_asset is positive, the trader is selling PT to receive asset, and the fee is positive (meaning they receive less asset)
/// If net_trader_asset is negative, the trader is selling asset to receive PT, and the fee is positive (meaning they pay more asset to receive PT)
pub fn asset_fee<N: Num>(net_trader_asset: N, fee_rate: N) -> N {
    assert!(fee_rate >= N::one());
    let is_sell_pt = net_trader_asset > N::zero();
    if is_sell_pt {
        // selling PT to buy asset
        net_trader_asset * (fee_rate - N::one())
    } else {
        // buying PT and spending asset
        -net_trader_asset * (fee_rate - N::one()) / fee_rate
    }
}

pub struct TradeResult<N: Num> {
    /// The change of asset for the trader after the fee is taken into account
    pub net_trader_asset: N,

    /// Positive asset fee
    ///
    /// When receiving asset, a trader gets *less* by the fee amount
    /// When sending asset, a trader sends *more* by the fee amount
    ///
    /// The reason for breaking out the fee separately is so that the product can
    /// take a cut of this fee to the treasury
    pub asset_fee: N,
}

pub fn trade<N: Num>(
    market_pt: u64,
    market_asset: u64,
    rate_scalar: N,
    rate_anchor: N,
    fee_rate: N,
    net_trader_pt: N,
    is_current_flash_swap: bool,
) -> TradeResult<N> {
    // assert that the user is selling PT into the market
    // or the market has more PT than the user is buying
    // (compared exactly, without truncating the trader's PT to an integer first)
    assert!(
        net_trader_pt < N::zero() || net_trader_pt < N::from_u64(market_pt),
        "trader cannot buy more PT than the market holds"
    );

    let market_pt = N::from_u64(market_pt);
    let new_pt = market_pt - net_trader_pt;

    let p = new_pt / (market_pt + N::from_u64(market_asset));

    // p is fed into logit(), which divides by (1 - p) and takes a logarithm.
    // p outside (0, 1) makes logit return inf or NaN, which the f64 backend
    // propagates silently (a trader can then be paid exactly zero). Fail loudly.
    assert!(
        p > N::zero() && p < N::one(),
        "post-trade PT proportion is out of the open interval (0, 1)"
    );

    let l_p = logit(p);
    let er = exchange_rate(l_p, rate_scalar, rate_anchor);

    assert!(er > N::one(), "Asset cannot be worth less than PT");

    // negate the trader PT to get the net change in asset for the trader
    let pre_fee_net_trader_asset = -net_trader_pt / er;

    // If the market is currently performing a flash swap, the fee is relative to the borrowed amount.
    let fee = if is_current_flash_swap {
        // 1 / er represents the PT price in asset terms
        // Therefore, (1 - (1/er)) represents the YT price in asset terms
        let yt_value = (N::one() - N::one() / er) * net_trader_pt.abs();
        asset_fee(yt_value, fee_rate)
    } else {
        asset_fee(pre_fee_net_trader_asset, fee_rate)
    };

    // subtract the fee from the net trader asset
    // if net_trader_asset is negative, the user is buying PT and selling asset and the "fee" value is positive in order to increase the magnitude of net_trader_asset (increasing the amount of asset the user must pay)
    // if net_trader_asset is positive, the user is selling PT and buying asset and the "fee" value is positive in order to decrease the magnitude of net_trader_asset
    let net_trader_asset = pre_fee_net_trader_asset - fee;

    TradeResult {
        net_trader_asset,
        asset_fee: fee,
    }
}
