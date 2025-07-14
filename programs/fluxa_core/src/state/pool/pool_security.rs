use anchor_lang::prelude::*;

/// Enhanced Pool Security with MEV protection and efficient tracking
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct PoolSecurity {
    /// Pool Account reference
    pub pool_core: Pubkey,

    /// Security flags packed into single u32 for efficiency
    pub security_flags: u32, // Bitfield: 0x01=MEV protection, 0x02=emergency pause, etc.

    /// Volume tracking with overflow protection
    /// 'total_swap_volume_0' - Total swap volume for token 0
    /// 'total_swap_volume_1' - Total swap volume for token 1
    /// 'active_positions_count' - Count of active positions for monitoring
    pub total_swap_volume_0: u64,
    pub total_swap_volume_1: u64,
    pub active_positions_count: u32,

    /// Security monitoring
    /// 'last_security_check' - Slot of last security check
    /// 'suspicious_activity_score' - Score for suspicious activity (0-1000)
    pub last_security_check: u64,
    pub suspicious_activity_score: u32,

    /// MEV protection parameters
    pub mev_protection_enabled: bool,
    // pub mev_protection_window: u16, // Slots to delay large trades
    // pub max_price_impact: u16,       // Basis points (e.g., 500 = 5%)
    // pub volume_spike_threshold: u32, // Multiplier for normal volume
    /// Emergency contacts for pool_core security
    pub emergency_contacts: Pubkey,

    /// Circuit breaker state
    /// 'circuit_breaker_triggered_at' - Slot when circuit breaker was triggered
    /// 'circuit_breaker_threshold' - Threshold for triggering circuit breaker (e.g.,
    pub circuit_breaker_triggered_at: u64,
    pub circuit_breaker_threshold: u32,

    /// Alignment and future expansion
    /// '_padding' - Padding for alignment for 8-byte boundary
    /// 'reserved' - Reserved space for future enhancements
    pub _padding: [u8; 4], // Align to 8-byte boundary
    pub reserved: [u64; 4],
}
