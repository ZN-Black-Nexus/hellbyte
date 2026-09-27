//! Fixed memory budgets. Level-derived limits are computed by the build
//! script from the largest shipped level so no RAM is wasted.

include!(concat!(env!("OUT_DIR"), "/limits.rs"));

/// Largest supported render resolution (the classic 320x200).
pub const MAX_W: usize = 320;
pub const MAX_H: usize = 200;

pub const MAX_MOBJS: usize = 512;
pub const MAX_MOVERS: usize = 48;
pub const MAX_LIGHTS: usize = 64;
pub const MAX_BUTTONS: usize = 16;
pub const MAX_SCROLLERS: usize = 16;
