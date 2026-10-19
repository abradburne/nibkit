//! The facts that differ between the systems nibkit runs on. Each module provides the same
//! items; the public modules read them from here.

#[cfg(feature = "nibos")]
mod nibos;
#[cfg(feature = "nibos")]
pub use nibos::*;

#[cfg(all(feature = "stock", not(feature = "nibos")))]
mod stock;
#[cfg(all(feature = "stock", not(feature = "nibos")))]
pub use stock::*;
