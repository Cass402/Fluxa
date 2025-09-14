#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

pub mod error;
pub mod math;
pub mod security;
pub mod state;
pub mod utils;

declare_id!("11111111111111111111111111111112");

#[program]
pub mod fluxa_core {}
