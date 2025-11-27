use crate::error::MathError;
use crate::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use crate::utils::constants::{MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK};
use anchor_lang::prelude::*;

const MAX_BINARY_ITERATIONS: usize = 32;
const BINARY_SEARCH_RANGE: i32 = 10_000;

#[cfg(test)]
pub(crate) const TEST_MAX_BINARY_ITERATIONS: usize = MAX_BINARY_ITERATIONS;

#[cfg(test)]
pub(crate) const TEST_BINARY_SEARCH_RANGE: i32 = BINARY_SEARCH_RANGE;

/// Converts a Q64x64 square root price to a normal price (u64), enforcing protocol bounds and safety.
///
/// # Why
/// This function is essential for translating between the protocol's internal fixed-point math (Q64x64) and user-facing price representations.
/// Squaring and shifting is used to avoid floating-point math, ensuring deterministic, overflow-resistant computation on-chain.
///
/// # Design Rationale
/// - Enforces that sqrt_price is within protocol-defined bounds, preventing invalid state or attacks.
/// - Uses checked arithmetic for all operations, ensuring that overflow/underflow cannot occur.
/// - Returns u64 to match SPL token and UI expectations, and to avoid accidental overflows in downstream logic.
///
/// # Trade-offs
/// - Shifting by 64 bits is safe because all protocol values are bounded and Q64x64 is used throughout.
/// - No dynamic allocation or floating-point math, ensuring deterministic and auditable execution.
///
/// # Usage
/// Used by the AMM and UI to display prices, and by protocol logic that needs to convert between price representations.
#[inline]
pub fn sqrt_price_to_price(sqrt_price: Q64x64) -> Result<u64> {
    // Safety: Enforce protocol bounds to prevent invalid state or attacks.
    if sqrt_price.raw() < MIN_SQRT_X64 || sqrt_price.raw() > MAX_SQRT_X64 {
        return Err(MathError::InvalidSqrtPrice.into());
    }

    // Rationale: Squaring and shifting is the canonical way to convert Q64x64 sqrt price to price, avoiding floating-point math.
    let price_x64 = sqrt_price.checked_mul(sqrt_price)?;
    let price = (price_x64.raw() >> 64) as u64;
    Ok(price)
}

/// Optimized binary search to find the tick index for a given sqrt price, minimizing branches for on-chain efficiency.
///
/// # Why
/// This function is used to invert the tick-to-sqrt mapping, which is nontrivial due to the nonlinearity of the mapping.
/// Binary search is used for efficiency, but is implemented in a branch-minimized way to reduce Solana compute cost and improve predictability.
///
/// # Design Rationale
/// - Uses a fixed iteration count to guarantee termination and avoid DoS vectors.
/// - Uses bitwise operations for mid calculation, avoiding division for performance.
/// - Uses branchless updates to low/high to reduce branch misprediction and improve runtime determinism.
/// - Returns the best guess (high) if an exact match is not found, which is safe for AMM logic.
///
/// # Usage
/// Used internally by sqrt_price_to_tick to refine the tick index after a coarse lookup.
#[inline(always)]
fn optimized_binary_search(sqrt_price: Q64x64, mut low: i32, mut high: i32) -> Result<i32> {
    for _ in 0..MAX_BINARY_ITERATIONS {
        // Early exit if search space is exhausted.
        if low >= high {
            break;
        }

        // Optimization: Bitwise mid calculation avoids division, which is expensive on-chain.
        let mid = low + ((high - low) >> 1);
        let mid_sqrt_price = tick_to_sqrt_x64(mid)?;

        // Branchless comparison and update for performance and predictability.
        let is_less = (mid_sqrt_price.raw() < sqrt_price.raw()) as u8;
        let is_equal = (mid_sqrt_price.raw() == sqrt_price.raw()) as u8;
        if is_equal == 1 {
            return Ok(mid);
        }
        low = if is_less == 1 { mid + 1 } else { low };
        high = if is_less == 0 { mid - 1 } else { high };
    }
    // Returns the best guess if no exact match, which is safe for AMM tick logic.
    Ok(high)
}

/// Converts a Q64x64 sqrt price to a tick index, using a hybrid coarse lookup and binary search for efficiency and safety.
///
/// # Why
/// This function is critical for mapping between price and tick space, which is the basis for all concentrated liquidity math.
/// The hybrid approach (coarse lookup + binary search) is used to minimize compute cost while ensuring correctness and determinism.
///
/// # Design Rationale
/// - Fast bounds check up front to prevent invalid state or attacks.
/// - Coarse lookup table provides a fast initial guess, reducing the search space for the binary search.
/// - Localized binary search ensures the result is precise, but with bounded compute cost.
/// - All math is checked and bounded, ensuring protocol safety and auditability.
///
/// # Usage
/// Used by the AMM and protocol logic to convert between price and tick representations, e.g., for position management and swaps.
#[inline(always)]
pub fn sqrt_price_to_tick(sqrt_price: Q64x64) -> Result<i32> {
    // Safety: Fast bounds check to prevent invalid state or attacks.
    if sqrt_price.raw() < MIN_SQRT_X64 || sqrt_price.raw() > MAX_SQRT_X64 {
        return Err(MathError::InvalidSqrtPrice.into());
    }

    // Optimization: Coarse lookup table provides a fast, deterministic initial guess, reducing compute cost.
    let coarse_tick = coarse_lookup_table_search(sqrt_price)?;

    // Rationale: Localized binary search ensures precision, but with bounded compute cost for on-chain safety.
    // The search range must be large enough to account for interpolation error in the coarse lookup.
    // With 10,000 tick gaps in the LOOKUP_TABLE, we need a proportional search range to ensure precision.
    let low = (coarse_tick - BINARY_SEARCH_RANGE).max(MIN_TICK);
    let high = (coarse_tick + BINARY_SEARCH_RANGE).min(MAX_TICK);

    optimized_binary_search(sqrt_price, low, high)
}

// Coarse lookup table for initial tick approximation.
//
// # Why
// This table is a protocol optimization: it allows for a fast, deterministic initial guess for tick index,
// reducing the search space for the binary search and minimizing compute cost on-chain.
//
// # Design Rationale
// - Precomputed and static, so it is zero-copy and does not require dynamic allocation.
// - Covers a wide range of sqrt prices, ensuring the binary search always starts close to the true tick.
// - Used only for initial approximation, so precision is not critical at this stage.
const LOOKUP_TABLE: &[(u128, i32)] = &[
    (4295048016u128, -443636),
    (7081160003u128, -433636),
    (11674567271u128, -423636),
    (19247626221u128, -413636),
    (31733177475u128, -403636),
    (52317856814u128, -393636),
    (86255407099u128, -383636),
    (142207569401u128, -373636),
    (234454783474u128, -363636),
    (386540925532u128, -353636),
    (637282314726u128, -343636),
    (1050674642287u128, -333636),
    (1732226328011u128, -323636),
    (2855886999350u128, -313636),
    (4708443937822u128, -303636),
    (7762717614757u128, -293636),
    (12798237711275u128, -283636),
    (21100199265645u128, -273636),
    (34787477705441u128, -263636),
    (57353420689108u128, -253636),
    (94557440829558u128, -243636),
    (155894966835576u128, -233636),
    (257020922641852u128, -223636),
    (423745269116628u128, -213636),
    (698620373987735u128, -203636),
    (1151801477260695u128, -193636),
    (1898952123951670u128, -183636),
    (3130764493927092u128, -173636),
    (5161628980954773u128, -163636),
    (8509874757016025u128, -153636),
    (14030060790363708u128, -143636),
    (23131081408573362u128, -133636),
    (38135752590433210u128, -123636),
    (62873654713769445u128, -113636),
    (103658540570089134u128, -103636),
    (170899768464829197u128, -93636),
    (281759039831204075u128, -83636),
    (464530509547998339u128, -73636),
    (765862186463289312u128, -63636),
    (1262661712413798865u128, -53636),
    (2081725182644421465u128, -43636),
    (3432098790555352899u128, -33636),
    (5658432825973808369u128, -23636),
    (9328945348008785685u128, -13636),
    (15380446138133159918u128, -3636),
    (25357434799262418241u128, 6364),
    (41806297023116804610u128, 16364),
    (68925208114343765373u128, 26364),
    (113635615969017952144u128, 36364),
    (187348773691563812079u128, 46364),
    (308878186688427609953u128, 56364),
    (509241306105368943433u128, 66364),
    (839575985032218442234u128, 76364),
    (1384192181961313676363u128, 86364),
    (2282090043975349204741u128, 96364),
    (3762436341341050932978u128, 106364),
    (6203053757679310631974u128, 116364),
    (10226851016154252775609u128, 126364),
    (16860805305312066838242u128, 136364),
    (27798073433805032898588u128, 146364),
    (45830129263622089444203u128, 156364),
    (75559220077677321405080u128, 166364),
    (124572979183774206252666u128, 176364),
    (205380986288206744641905u128, 186364),
    (338607535960822095323750u128, 196364),
    (558255491326575044447879u128, 206364),
    (920384694664133076213866u128, 216364),
    (1517419889876977501527448u128, 226364),
    (2501739908913316094565612u128, 236364),
    (4124568692952233448943249u128, 246364),
    (6800094143388090358257202u128, 256364),
    (11211179592657720290718612u128, 266364),
    (18483648197876368932281749u128, 276364),
    (30473622144685310225620832u128, 286364),
    (50241253061922527412842682u128, 296364),
    (82831751908178227542630197u128, 306364),
    (136563057368845036093123534u128, 316364),
    (225148789060987402398075202u128, 326364),
    (371198318141884767117756300u128, 336364),
    (611987263915687258264906382u128, 346364),
    (1008971196501626940274428339u128, 356364),
    (1663470688681156048631958581u128, 366364),
    (2742530948054568822487737536u128, 376364),
    (4521556076831338071631713703u128, 386364),
    (7454599325645827675584627361u128, 396364),
    (12290249233149591626786744287u128, 406364),
    (20262688793116043139907801508u128, 416364),
    (33406690892748727032575218928u128, 426364),
    (55076945009529438865748214840u128, 436364),
];

/// Coarse lookup in the precomputed table to get an initial tick index guess for a given sqrt price.
///
/// # Why
/// This function is a protocol optimization: it provides a fast, deterministic initial guess for the tick index,
/// reducing the search space for the binary search and minimizing compute cost on-chain.
///
/// # Design Rationale
/// - Uses binary search for O(log n) lookup, which is efficient and deterministic.
/// - Returns the closest lower tick if no exact match, which is safe for AMM logic.
/// - Interpolates between table entries for better accuracy, but only as an initial guess.
///
/// # Usage
/// Used internally by sqrt_price_to_tick for fast initial tick approximation.
#[inline(always)]
fn coarse_lookup_table_search(sqrt_price: Q64x64) -> Result<i32> {
    // Rationale: Binary search for O(log n) lookup, which is efficient and deterministic.
    match LOOKUP_TABLE.binary_search_by_key(&sqrt_price.raw(), |&(price, _)| price) {
        Ok(index) => Ok(LOOKUP_TABLE[index].1), // Exact match found
        Err(index) => {
            // If no exact match, return the closest lower tick, which is safe for AMM logic.
            if index == 0 {
                Ok(LOOKUP_TABLE[0].1)
            } else if index >= LOOKUP_TABLE.len() {
                Ok(LOOKUP_TABLE[LOOKUP_TABLE.len() - 1].1)
            } else {
                // Interpolate for better accuracy, but only as an initial guess.
                let (lower_price, lower_tick) = LOOKUP_TABLE[index - 1];
                let (upper_price, upper_tick) = LOOKUP_TABLE[index];
                if upper_price == lower_price {
                    Ok(lower_tick)
                } else {
                    //let weight = (sqrt_price.raw() - lower_price) / (upper_price - lower_price);
                    let numerator = sqrt_price.checked_sub(Q64x64::from_raw(lower_price))?;
                    let denominator =
                        Q64x64::from_raw(upper_price).checked_sub(Q64x64::from_raw(lower_price))?;
                    let weight = numerator.checked_div(denominator)?.raw();
                    Ok(lower_tick + ((weight * (upper_tick - lower_tick) as u128) >> 64) as i32)
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn lookup_table_for_tests() -> &'static [(u128, i32)] {
    LOOKUP_TABLE
}

#[cfg(test)]
pub(crate) fn coarse_lookup_table_search_for_tests(sqrt_price: Q64x64) -> Result<i32> {
    coarse_lookup_table_search(sqrt_price)
}

#[cfg(test)]
pub(crate) fn optimized_binary_search_for_tests(
    sqrt_price: Q64x64,
    low: i32,
    high: i32,
) -> Result<i32> {
    optimized_binary_search(sqrt_price, low, high)
}
