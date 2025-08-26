use crate::{math::core_arithmetic::Q64x64, utils::constants::SECURITY_FLAG_DEFAULT};
use anchor_lang::prelude::*;

/// Security and risk management state for a concentrated liquidity pool.
///
/// # Why this structure?
/// - Uses zero-copy layout for maximum on-chain efficiency and deterministic account size, critical for Solana's rent and compute model.
/// - All fields are fixed-size and aligned, with no dynamic allocations, ensuring safety and predictable performance.
/// - Packs all security, MEV protection, and circuit breaker logic into a single account for atomic updates and easier auditing.
/// - Bitfields and booleans are used for efficient flag management, minimizing storage and compute costs.
///
/// ## Usage
/// This struct is the canonical security and risk control state for a pool, referenced by all swap, admin, and monitoring logic.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)]
#[repr(C)]
pub struct PoolSecurity {
    /// Reference to the associated PoolCore account.
    ///
    /// Why: Ensures this security state is always bound to a specific pool, preventing misconfiguration or spoofing. Used for Anchor constraint validation.
    pub pool_core: Pubkey,

    /// Security flags packed into a single u32 bitfield.
    ///
    /// Why: Bitfields allow multiple security statuses (e.g., MEV protection, emergency pause) to be tracked compactly and atomically, minimizing storage and compute. Enables efficient flag checks and updates.
    pub security_flags: u32,

    /// Volume and position tracking for risk monitoring.
    ///
    /// - `total_swap_volume_0`/`total_swap_volume_1`: Cumulative swap volume for each token, in Q64.64. Why: Enables detection of abnormal activity and supports circuit breaker logic. Fixed-point ensures precision and overflow safety.
    /// - `active_positions_count`: Number of active LP positions. Why: Used for monitoring pool health and potential attack surface.
    pub active_positions_count: u32,
    pub _padding1: [u8; 8],
    pub total_swap_volume_0: Q64x64,
    pub total_swap_volume_1: Q64x64,

    /// Security monitoring and anomaly detection.
    ///
    /// - `last_security_check`: Last slot when security checks were performed. Why: Enables time-based logic and replay protection.
    /// - `suspicious_activity_score`: Composite score for suspicious activity (0-1000). Why: Allows for nuanced threat detection and automated circuit breaker triggers.
    pub last_security_check: u64,
    pub suspicious_activity_score: u32,

    /// MEV protection parameters and emergency contacts.
    ///
    /// - `mev_protection_enabled`: Enables/disables MEV protection logic. Why: Allows for dynamic risk management and protocol upgrades without redeploying the contract.
    /// - `emergency_contacts`: Pubkey for emergency response (e.g., multisig, DAO). Why: Enables rapid intervention in case of attack or critical failure.
    pub mev_protection_enabled: u8, // 0 = disabled, 1 = enabled
    // Future: Add MEV protection window, price impact, and volume spike thresholds for more granular controls.
    pub emergency_contacts: Pubkey,
    pub security_coordinator: Pubkey,

    /// Oracle confirmation configuration for smart hybrid validation.
    ///
    /// - `oracle_confirmation_mode`: How oracles are used for validation. Why: Enables tiered security model from oracle-free to oracle-enhanced.
    /// - `oracle_feed`: Optional oracle feed for price confirmation. Why: Provides "second opinion" without making pools oracle-dependent.
    /// - `false_positive_count`: Track internal system accuracy. Why: Helps tune internal detection sensitivity over time.
    pub oracle_confirmation_mode: u8, // 0 = disabled, 1 = attack_confirmation, 2 = advisory
    pub oracle_feed: Pubkey, // Optional oracle for confirmation
    pub _padding2: [u8; 2],
    pub false_positive_count: u32, // Track internal detection accuracy
    pub oracle_confirmation_count: u32, // Track oracle confirmations

    /// Circuit breaker state and configuration.
    ///
    /// - `circuit_breaker_triggered_at`: Slot when circuit breaker was last triggered. Why: Enables time-based lockouts and post-mortem analysis.
    /// - `circuit_breaker_threshold`: Threshold for triggering circuit breaker (e.g., suspicious activity score, volume spike). Why: Allows for flexible, automated risk controls.
    pub circuit_breaker_triggered_at: u64,
    pub circuit_breaker_threshold: u32,

    /// Alignment and future expansion.
    ///
    /// - `_padding`: Ensures 8-byte alignment for Anchor zero-copy safety and future extensibility.
    /// - `reserved`: Pre-allocated space for future upgrades (e.g., new risk controls, monitoring fields) without breaking account layout.
    pub bump_security: u8, // Cache bump for efficiency
    pub enterprise_mode: u8, // Whether this pool is in enterprise mode with enhanced security/compliance
    // Expanded to 10 bytes to maintain alignment before `reserved` and avoid implicit compiler padding.
    // Without this, Rust would insert 6 bytes of hidden padding before `reserved` (u64 alignment),
    // causing a size mismatch for Anchor's zero_copy safety check.
    pub _padding3: [u8; 10],
    pub reserved: [u64; 4],
}

impl Default for PoolSecurity {
    fn default() -> Self {
        Self {
            pool_core: Pubkey::default(),
            security_flags: SECURITY_FLAG_DEFAULT,
            active_positions_count: 0,
            _padding1: [0; 8],
            total_swap_volume_0: Q64x64::zero(),
            total_swap_volume_1: Q64x64::zero(),
            last_security_check: 0,
            suspicious_activity_score: 0,
            mev_protection_enabled: 1,
            emergency_contacts: Pubkey::default(),
            security_coordinator: Pubkey::default(),
            oracle_confirmation_mode: 1, // Default to attack confirmation mode
            oracle_feed: Pubkey::default(),
            _padding2: [0; 2],
            false_positive_count: 0,
            oracle_confirmation_count: 0,
            circuit_breaker_triggered_at: 0,
            circuit_breaker_threshold: 0,
            bump_security: 0,
            enterprise_mode: 0,
            _padding3: [0; 10],
            reserved: [0; 4],
        }
    }
}

impl PoolSecurity {
    /// Validate a swap using smart oracle confirmation strategy
    ///
    /// This is the core of Fluxa's hybrid security model:
    /// - Normal pools: Oracle-free by default, oracle confirms only when internal systems detect threats
    /// - Enterprise pools: Can use full oracle validation if desired
    pub fn validate_swap_with_smart_confirmation(
        &mut self,
        swap_amount: Q64x64,
        current_price: Q64x64,
        oracle_price: Option<u64>,
        clock: &Clock,
    ) -> Result<bool> {
        // Step 1: Run internal threat detection (oracle-free)
        let internal_result = self.analyze_internal_threats(swap_amount, current_price, clock)?;

        // Step 2: Apply smart oracle confirmation based on mode
        let validation_result = crate::state::pool::smart_oracle_confirmation::SmartOracleConfirmation::validate_with_smart_confirmation(
            self,
            &internal_result,
            current_price,
            oracle_price,
        )?;

        // Step 3: Update statistics for continuous improvement
        if oracle_price.is_some()
            && internal_result.threat_level
                != crate::state::pool::smart_oracle_confirmation::ThreatLevel::Clean
        {
            // We'll track actual outcomes in a follow-up transaction
            self.oracle_confirmation_count += 1;
        }

        // Step 4: Update security state
        self.last_security_check = clock.slot;
        self.suspicious_activity_score = internal_result.confidence as u32;

        Ok(validation_result)
    }

    /// Internal threat detection that runs oracle-free
    fn analyze_internal_threats(
        &self,
        swap_amount: Q64x64,
        _current_price: Q64x64,
        clock: &Clock,
    ) -> Result<crate::state::pool::smart_oracle_confirmation::InternalSecurityResult> {
        let mut threat_level = crate::state::pool::smart_oracle_confirmation::ThreatLevel::Clean;
        let mut confidence = 0u8;
        let mut detected_attacks = 0u32;
        let price_deviation = 0u32;
        let mut volume_anomaly = 0u32;

        // Check 1: Large swap detection
        let swap_amount_u64 = (swap_amount.raw() >> 32) as u64;
        if swap_amount_u64 > 1_000_000_000 {
            // Large swap threshold
            confidence += 20;
            volume_anomaly += 30;
            detected_attacks |=
                crate::state::pool::smart_oracle_confirmation::ATTACK_TYPE_PRICE_MANIPULATION;
        }

        // Check 2: Rapid transaction pattern (simplified - would use more sophisticated detection)
        let time_since_last_check = clock.slot.saturating_sub(self.last_security_check);
        if time_since_last_check < 5 {
            // Less than 5 slots between swaps
            confidence += 30;
            detected_attacks |= crate::state::pool::smart_oracle_confirmation::ATTACK_TYPE_SANDWICH;
        }

        // Check 3: Historical suspicious activity
        if self.suspicious_activity_score > 500 {
            confidence += 25;
        }

        // Check 4: Circuit breaker proximity
        if self.suspicious_activity_score > self.circuit_breaker_threshold.saturating_sub(100) {
            confidence += 35;
            threat_level = crate::state::pool::smart_oracle_confirmation::ThreatLevel::Suspicious;
        }

        // Determine final threat level
        if confidence > 80 {
            threat_level = crate::state::pool::smart_oracle_confirmation::ThreatLevel::HighRisk;
        } else if confidence > 50 {
            threat_level = crate::state::pool::smart_oracle_confirmation::ThreatLevel::Suspicious;
        }

        Ok(
            crate::state::pool::smart_oracle_confirmation::InternalSecurityResult {
                threat_level,
                confidence,
                detected_attack_types: detected_attacks,
                price_deviation,
                volume_anomaly_score: volume_anomaly,
                timestamp: clock.unix_timestamp,
            },
        )
    }

    /// Check if circuit breaker should be triggered
    pub fn should_trigger_circuit_breaker(&self) -> bool {
        self.suspicious_activity_score > self.circuit_breaker_threshold
    }

    /// Trigger circuit breaker
    pub fn trigger_circuit_breaker(&mut self, clock: &Clock) {
        self.circuit_breaker_triggered_at = clock.slot;
        self.security_flags |= 0x01; // Set emergency pause bit
        msg!("Circuit breaker triggered at slot {}", clock.slot);
    }

    /// Check if pool is in emergency pause
    pub fn is_emergency_paused(&self) -> bool {
        (self.security_flags & 0x01) != 0
    }

    /// Reset circuit breaker (admin only)
    pub fn reset_circuit_breaker(&mut self) {
        self.security_flags &= !0x01; // Clear emergency pause bit
        self.suspicious_activity_score = 0;
        msg!("Circuit breaker reset");
    }
}
