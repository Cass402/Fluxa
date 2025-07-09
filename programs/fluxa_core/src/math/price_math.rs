use crate::error::MathError;
use crate::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use crate::utils::constants::{MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK};
use anchor_lang::prelude::*;

/// Convert sqrt price (which is a Q64x64 value) to the normal price (which is a u64 value).
/// The normal price is calculated by squaring the square root price and shifting the result right by 64 bits.
/// This function checks if the sqrt price is within valid bounds before performing the conversion.
/// # Arguments
/// * `sqrt_price` - A Q64x64 value representing the square root price.
/// # Returns
/// * `Result<u64>` - The converted price as a u64 value if successful,
///   or an error if the sqrt price is out of bounds.
/// # Errors
/// * `MathError::InvalidSqrtPrice` - If the sqrt price is less than `MIN_SQRT_X64` or greater than `MAX_SQRT_X64`.
#[inline]
pub fn sqrt_price_to_price(sqrt_price: Q64x64) -> Result<u64> {
    // Check if the sqrt price is within the valid range
    if sqrt_price.raw() < MIN_SQRT_X64 || sqrt_price.raw() > MAX_SQRT_X64 {
        return Err(MathError::InvalidSqrtPrice.into());
    }

    // Calculate the price by squaring the sqrt price
    let price_x64 = sqrt_price.checked_mul(sqrt_price)?;

    // Shift the result right by 64 bits to convert from Q64x64 to u64
    // This effectively divides the squared value by 2^64, yielding the normal price
    // The raw() method retrieves the underlying u128 value from the Q64x64 type
    let price = (price_x64.raw() >> 64) as u64;

    Ok(price)
}

/// Perform a binary search to find the tick index corresponding to a given square root price.
/// This function uses an optimized binary search algorithm to efficiently locate the tick index.
/// # Arguments
/// * `sqrt_price` - A Q64x64 value representing the square root price to search for.
/// * `low` - The lower bound of the tick index range to search within.
/// * `high` - The upper bound of the tick index range to search within.
/// # Returns
/// * `Result<i32>` - The tick index corresponding to the square root price if found,
///   or an error if the search fails or the price is out of bounds.
/// # Errors
/// * `MathError::InvalidSqrtPrice` - If the square root price is less
#[inline(always)]
fn optimized_binary_search(sqrt_price: Q64x64, mut low: i32, mut high: i32) -> Result<i32> {
    // The MAX_BINARY_ITERATIONS constant defines the maximum number of iterations for the binary search.
    // this ensures that the search converges quickly and avoids infinite loops.
    const MAX_BINARY_ITERATIONS: usize = 32;

    for _ in 0..MAX_BINARY_ITERATIONS {
        // If the low index is greater than or equal to the high index, break the loop.
        if low >= high {
            break;
        }

        // Optimized calculation of the mid index using bitwise operations instead of division.
        let mid = low + ((high - low) >> 1);
        let mid_sqrt_price = tick_to_sqrt_x64(mid)?;

        // Branchless comparision using conditional moves
        // Reduces branch mispredictions and improves performance
        let is_less = (mid_sqrt_price.raw() < sqrt_price.raw()) as u8;
        let is_equal = (mid_sqrt_price.raw() == sqrt_price.raw()) as u8;

        // Early exit on exact match
        if is_equal == 1 {
            return Ok(mid);
        }

        // Branchless update: if is_less, update low; otherwise, update high
        low = if is_less == 1 { mid + 1 } else { low };
        high = if is_less == 0 { mid - 1 } else { high };
    }

    Ok(high)
}

/// Convert a square root price (Q64x64) to a tick index.
/// This function first performs a coarse lookup using a lookup table to find an initial tick index,
/// then refines the result using a localized binary search.
/// # Arguments
/// * `sqrt_price` - A Q64x64 value representing the square root price to convert.
/// # Returns
/// * `Result<i32>` - The tick index corresponding to the square root price if successful,
///   or an error if the square root price is out of bounds.
/// # Errors
/// * `MathError::InvalidSqrtPrice` - If the square root price is less than `MIN_SQRT_X64` or greater than `MAX_SQRT_X64`.
#[inline(always)]
pub fn sqrt_price_to_tick(sqrt_price: Q64x64) -> Result<i32> {
    // Fast bounds check
    if sqrt_price.raw() < MIN_SQRT_X64 || sqrt_price.raw() > MAX_SQRT_X64 {
        return Err(MathError::InvalidSqrtPrice.into());
    }

    // Use coarse lookup table for initial approximation
    let coarse_tick = coarse_lookup_table_search(sqrt_price);

    // Refine with localized binary search
    let search_range = 10; // Small range around the coarse tick
    let low = (coarse_tick - search_range).max(MIN_TICK); // Ensure low does not go below MIN_TICK
    let high = (coarse_tick + search_range).min(MAX_TICK); // Ensure high does not exceed MAX_TICK

    optimized_binary_search(sqrt_price, low, high)
}

// Coarse lookup table for initial tick approximation
// This table maps square root prices to tick indices, allowing for a quick initial guess
// The values are precomputed to cover a wide range of square root prices, improving search efficiency
// The table is designed to be used with the `coarse_lookup_table_search` function,
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

/// Perform a coarse lookup in the precomputed lookup table to find the tick index for a given square root price.
/// This function uses a binary search on the lookup table to find the closest tick index.
/// # Arguments
/// * `sqrt_price` - A Q64x64 value representing the square root price to search for.
/// # Returns
/// * `i32` - The tick index corresponding to the square root price.
/// If the square root price is not found in the lookup table, it returns the closest tick index.
/// # Notes
/// This function is designed to be efficient for initial tick approximation, allowing for quick lookups
/// before performing a more precise search if necessary.
#[inline(always)]
fn coarse_lookup_table_search(sqrt_price: Q64x64) -> i32 {
    // Perform a binary search on the lookup table to find the closest tick index
    match LOOKUP_TABLE.binary_search_by_key(&sqrt_price.raw(), |&(price, _)| price) {
        Ok(index) => LOOKUP_TABLE[index].1, //Exact match found
        Err(index) => {
            // If no exact match is found, return the tick index of the closest lower value
            if index == 0 {
                // If the index is 0, return the first tick index
                LOOKUP_TABLE[0].1
            } else if index >= LOOKUP_TABLE.len() {
                // If the index is out of bounds, return the last tick index
                LOOKUP_TABLE[LOOKUP_TABLE.len() - 1].1
            } else {
                // Interpolate between the two closest values
                let (lower_price, lower_tick) = LOOKUP_TABLE[index - 1]; // Get the lower value
                let (upper_price, upper_tick) = LOOKUP_TABLE[index]; // Get the upper value

                if upper_price == lower_price {
                    // If the prices are equal, return the lower tick index
                    lower_tick
                } else {
                    // Calculate the weight for interpolation
                    // This is a linear interpolation between the lower and upper tick indices based on the square root
                    // price's position between the lower and upper prices
                    let weight = (sqrt_price.raw() - lower_price) / (upper_price - lower_price);
                    // Interpolate the tick index using the weight
                    lower_tick + (weight * (upper_tick - lower_tick) as u128) as i32
                }
            }
        }
    }
}
