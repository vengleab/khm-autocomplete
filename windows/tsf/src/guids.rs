//! Identifiers of the text service. Change these if you fork the product: they must be unique to it.
use windows::core::GUID;

/// the text service (COM class)
pub const CLSID_KHMER_TIP: GUID = GUID::from_u128(0xF72DDC90_B765_436E_AF1A_E68BDD215D1E);
/// its language profile
pub const GUID_KHMER_PROFILE: GUID = GUID::from_u128(0x9B6B60CE_921C_4D37_B157_5104F3A3CC96);
/// Khmer (Cambodia)
pub const LANGID_KHMER: u16 = 0x0453;
pub const DISPLAY_NAME: &str = "Khmer Autocomplete";
pub const REG_SETTINGS_KEY: &str = r"Software\KhmerAutocomplete";
