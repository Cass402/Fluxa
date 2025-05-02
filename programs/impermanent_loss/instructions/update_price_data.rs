use crate::*;
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<UpdatePriceData>, price: u64, timestamp: i64) -> Result<()> {
    let price_history = &mut ctx.accounts.price_history;

    // Validate timestamp
    require!(timestamp > 0, ErrorCode::InvalidPriceData);
    require!(price > 0, ErrorCode::InvalidPriceData);

    // Get current index for circular buffer
    let current_idx = price_history.current_index as usize;

    // Update price and timestamp at the current index
    price_history.prices[current_idx] = price;
    price_history.timestamps[current_idx] = timestamp;

    // Increment index and wrap around if needed
    price_history.current_index = ((current_idx + 1) % price_history.prices.len()) as u16;

    // Update data count for initial filling
    if price_history.data_count < price_history.prices.len() as u16 {
        price_history.data_count += 1;
    }

    Ok(())
}
