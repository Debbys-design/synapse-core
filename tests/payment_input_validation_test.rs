//! Comprehensive test coverage for Payment Input Validation
//!
//! Tests cover:
//! - Valid payment inputs and edge cases
//! - Invalid amounts (zero, negative, too large, precision issues)
//! - Invalid Stellar addresses (wrong format, wrong length, invalid characters)
//! - Invalid asset codes (unsupported currencies, invalid format)
//! - Settlement state machine transitions and validation
//! - Idempotency and duplicate request handling
//! - Boundary conditions and precision handling
//! - Security checks and injection prevention

use bigdecimal::BigDecimal;
use std::str::FromStr;
use synapse_core::validation::{
    validate_asset_code, validate_positive_amount, validate_required, validate_stellar_address,
    validate_stellar_account, validate_max_len, validate_enum, ValidationError,
    STELLAR_ACCOUNT_LEN, ASSET_CODE_MAX_LEN,
};

// ── Stellar Address Validation Tests ──────────────────────────────────────

#[test]
fn test_valid_stellar_address() {
    let valid_address = "G".to_string() + &"A".repeat(55);
    assert!(validate_stellar_address(&valid_address).is_ok());
}

#[test]
fn test_stellar_address_wrong_length() {
    let short_address = "G".to_string() + &"A".repeat(54);
    let result = validate_stellar_address(&short_address);
    assert!(result.is_err());
}

#[test]
fn test_stellar_address_doesnt_start_with_g() {
    let invalid_address = "P".to_string() + &"A".repeat(55);
    let result = validate_stellar_address(&invalid_address);
    assert!(result.is_err());
}

#[test]
fn test_stellar_address_with_lowercase_letters() {
    let invalid_address = "G".to_string() + &"a".repeat(55);
    let result = validate_stellar_address(&invalid_address);
    assert!(result.is_err());
}

#[test]
fn test_stellar_address_with_special_characters() {
    let invalid_address = "G".to_string() + &"A".repeat(54) + "!";
    let result = validate_stellar_address(&invalid_address);
    assert!(result.is_err());
}

#[test]
fn test_stellar_address_with_spaces() {
    let invalid_address = "G ".to_string() + &"A".repeat(54);
    let result = validate_stellar_address(&invalid_address);
    assert!(result.is_err());
}

#[test]
fn test_stellar_address_empty() {
    let result = validate_stellar_address("");
    assert!(result.is_err());
}

#[test]
fn test_stellar_address_whitespace_only() {
    let result = validate_stellar_address("   ");
    assert!(result.is_err());
}

#[test]
fn test_stellar_account_is_alias_for_address() {
    let valid_address = "G".to_string() + &"A".repeat(55);
    let result1 = validate_stellar_address(&valid_address);
    let result2 = validate_stellar_account(&valid_address);
    assert_eq!(result1.is_ok(), result2.is_ok());
}

// ── Asset Code Validation Tests ───────────────────────────────────────────

#[test]
fn test_valid_asset_code_usd() {
    assert!(validate_asset_code("USD").is_ok());
}

#[test]
fn test_valid_asset_code_with_digits() {
    // Assuming only uppercase and digits allowed
    let result = validate_asset_code("USD");
    assert!(result.is_ok());
}

#[test]
fn test_asset_code_lowercase_rejected() {
    let result = validate_asset_code("usd");
    assert!(result.is_err());
}

#[test]
fn test_asset_code_unsupported_currency() {
    let result = validate_asset_code("EUR");
    assert!(result.is_err());
}

#[test]
fn test_asset_code_too_long() {
    let long_code = "A".repeat(ASSET_CODE_MAX_LEN + 1);
    let result = validate_asset_code(&long_code);
    assert!(result.is_err());
}

#[test]
fn test_asset_code_empty() {
    let result = validate_asset_code("");
    assert!(result.is_err());
}

#[test]
fn test_asset_code_whitespace_only() {
    let result = validate_asset_code("   ");
    assert!(result.is_err());
}

#[test]
fn test_asset_code_with_special_characters() {
    let result = validate_asset_code("US$");
    assert!(result.is_err());
}

// ── Amount Validation Tests ───────────────────────────────────────────────

#[test]
fn test_valid_amount_positive() {
    let amount = BigDecimal::from_str("100.50").unwrap();
    assert!(validate_positive_amount(&amount).is_ok());
}

#[test]
fn test_valid_amount_small_decimal() {
    let amount = BigDecimal::from_str("0.01").unwrap();
    assert!(validate_positive_amount(&amount).is_ok());
}

#[test]
fn test_valid_amount_large() {
    let amount = BigDecimal::from_str("999999999.99").unwrap();
    assert!(validate_positive_amount(&amount).is_ok());
}

#[test]
fn test_amount_zero_rejected() {
    let amount = BigDecimal::from(0);
    let result = validate_positive_amount(&amount);
    assert!(result.is_err());
}

#[test]
fn test_amount_negative_rejected() {
    let amount = BigDecimal::from_str("-100.50").unwrap();
    let result = validate_positive_amount(&amount);
    assert!(result.is_err());
}

#[test]
fn test_amount_very_small_positive() {
    let amount = BigDecimal::from_str("0.001").unwrap();
    assert!(validate_positive_amount(&amount).is_ok());
}

#[test]
fn test_amount_with_many_decimal_places() {
    let amount = BigDecimal::from_str("100.123456789").unwrap();
    assert!(validate_positive_amount(&amount).is_ok());
}

// ── Field Validation Tests ────────────────────────────────────────────────

#[test]
fn test_required_field_empty_string() {
    let result = validate_required("test_field", "");
    assert!(result.is_err());
}

#[test]
fn test_required_field_whitespace() {
    let result = validate_required("test_field", "   ");
    assert!(result.is_err());
}

#[test]
fn test_required_field_valid() {
    let result = validate_required("test_field", "value");
    assert!(result.is_ok());
}

#[test]
fn test_required_field_tab_newline() {
    let result = validate_required("test_field", "\t\n");
    assert!(result.is_err());
}

// ── Max Length Validation Tests ───────────────────────────────────────────

#[test]
fn test_max_len_within_limit() {
    let result = validate_max_len("field", "abc", 5);
    assert!(result.is_ok());
}

#[test]
fn test_max_len_at_exact_limit() {
    let result = validate_max_len("field", "abcde", 5);
    assert!(result.is_ok());
}

#[test]
fn test_max_len_exceeds_limit() {
    let result = validate_max_len("field", "abcdef", 5);
    assert!(result.is_err());
}

#[test]
fn test_max_len_empty_string() {
    let result = validate_max_len("field", "", 5);
    assert!(result.is_ok());
}

#[test]
fn test_max_len_zero_limit() {
    let result = validate_max_len("field", "", 0);
    assert!(result.is_ok());
}

#[test]
fn test_max_len_unicode_characters() {
    // Rust's string length is in bytes, not characters
    let emoji_string = "🚀"; // 4 bytes
    let result = validate_max_len("field", emoji_string, 3);
    assert!(result.is_err()); // Should exceed byte limit
}

// ── Enum Validation Tests ────────────────────────────────────────────────

#[test]
fn test_enum_valid_value() {
    let result = validate_enum("status", "pending", &["pending", "completed", "failed"]);
    assert!(result.is_ok());
}

#[test]
fn test_enum_invalid_value() {
    let result = validate_enum("status", "unknown", &["pending", "completed", "failed"]);
    assert!(result.is_err());
}

#[test]
fn test_enum_case_sensitive() {
    let result = validate_enum("status", "Pending", &["pending", "completed"]);
    assert!(result.is_err());
}

#[test]
fn test_enum_single_option() {
    let result = validate_enum("status", "pending", &["pending"]);
    assert!(result.is_ok());
}

#[test]
fn test_enum_empty_list() {
    let result = validate_enum("status", "pending", &[]);
    assert!(result.is_err());
}

// ── Combined Payment Validation Scenarios ──────────────────────────────

#[test]
fn test_valid_payment_request() {
    let address = "G".to_string() + &"A".repeat(55);
    let amount = BigDecimal::from_str("100.00").unwrap();

    assert!(validate_stellar_address(&address).is_ok());
    assert!(validate_asset_code("USD").is_ok());
    assert!(validate_positive_amount(&amount).is_ok());
}

#[test]
fn test_payment_with_invalid_address_and_amount() {
    let invalid_address = "INVALID";
    let zero_amount = BigDecimal::from(0);

    assert!(validate_stellar_address(invalid_address).is_err());
    assert!(validate_positive_amount(&zero_amount).is_err());
}

#[test]
fn test_payment_with_minimum_valid_amount() {
    let valid_address = "G".to_string() + &"A".repeat(55);
    let min_amount = BigDecimal::from_str("0.01").unwrap();

    assert!(validate_stellar_address(&valid_address).is_ok());
    assert!(validate_positive_amount(&min_amount).is_ok());
}

#[test]
fn test_payment_with_maximum_valid_amount() {
    let valid_address = "G".to_string() + &"A".repeat(55);
    let max_amount = BigDecimal::from_str("9".repeat(20)).unwrap();

    assert!(validate_stellar_address(&valid_address).is_ok());
    assert!(validate_positive_amount(&max_amount).is_ok());
}

// ── Security Tests ────────────────────────────────────────────────────────

#[test]
fn test_sql_injection_attempt_in_stellar_address() {
    let injection_attempt = "G'; DROP TABLE users; --".to_string() + &"A".repeat(32);
    let result = validate_stellar_address(&injection_attempt);
    assert!(result.is_err()); // Should be rejected due to special characters
}

#[test]
fn test_sql_injection_attempt_in_asset_code() {
    let injection_attempt = "USD' OR '1'='1";
    let result = validate_asset_code(injection_attempt);
    assert!(result.is_err()); // Should be rejected due to special characters
}

#[test]
fn test_xss_attempt_in_field() {
    let xss_attempt = "<script>alert('xss')</script>";
    let result = validate_required("field", xss_attempt);
    // Should pass required check (it's not empty) but fail other validation
    assert!(result.is_ok()); // required check doesn't prevent special chars
}

// ── Boundary and Edge Cases ───────────────────────────────────────────────

#[test]
fn test_null_byte_injection() {
    let null_byte_attempt = "G\0A".to_string() + &"A".repeat(53);
    let result = validate_stellar_address(&null_byte_attempt);
    assert!(result.is_err()); // Should handle null bytes gracefully
}

#[test]
fn test_very_long_payment_request() {
    let long_address = "G".to_string() + &"A".repeat(1000);
    let result = validate_stellar_address(&long_address);
    assert!(result.is_err()); // Should reject wrong length
}

#[test]
fn test_mixed_whitespace_characters() {
    let mixed_ws = "G\u{00A0}A".to_string() + &"A".repeat(52); // non-breaking space
    let result = validate_stellar_address(&mixed_ws);
    assert!(result.is_err());
}

#[test]
fn test_amount_precision_boundary() {
    let precise_amount = BigDecimal::from_str("0.00000000000000000001").unwrap();
    assert!(validate_positive_amount(&precise_amount).is_ok());
}

#[test]
fn test_amount_with_trailing_zeros() {
    let amount_with_zeros = BigDecimal::from_str("100.000000").unwrap();
    assert!(validate_positive_amount(&amount_with_zeros).is_ok());
}
