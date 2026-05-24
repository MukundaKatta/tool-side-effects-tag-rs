//! Integration tests for tool-side-effects-tag.

use std::str::FromStr;

use tool_side_effects_tag::{
    is_destructive, is_parallel_safe, is_retry_safe, HasSideEffects, ParseSideEffectError,
    SideEffect, SideEffects, Tag,
};

// ---- enum variants ------------------------------------------------------

#[test]
fn enum_variants_have_stable_string_form() {
    assert_eq!(SideEffect::Read.as_str(), "read");
    assert_eq!(SideEffect::Write.as_str(), "write");
    assert_eq!(SideEffect::Idempotent.as_str(), "idempotent");
    assert_eq!(SideEffect::Destructive.as_str(), "destructive");
    assert_eq!(SideEffect::External.as_str(), "external");
    assert_eq!(SideEffect::Expensive.as_str(), "expensive");
    assert_eq!(SideEffect::Network.as_str(), "network");
}

#[test]
fn enum_display_matches_str_slug() {
    assert_eq!(format!("{}", SideEffect::Read), "read");
    assert_eq!(format!("{}", SideEffect::Destructive), "destructive");
    assert_eq!(format!("{}", SideEffect::Network), "network");
}

// ---- FromStr ------------------------------------------------------------

#[test]
fn from_str_parses_every_variant() {
    assert_eq!(SideEffect::from_str("read").unwrap(), SideEffect::Read);
    assert_eq!(SideEffect::from_str("write").unwrap(), SideEffect::Write);
    assert_eq!(
        SideEffect::from_str("idempotent").unwrap(),
        SideEffect::Idempotent
    );
    assert_eq!(
        SideEffect::from_str("destructive").unwrap(),
        SideEffect::Destructive
    );
    assert_eq!(
        SideEffect::from_str("external").unwrap(),
        SideEffect::External
    );
    assert_eq!(
        SideEffect::from_str("expensive").unwrap(),
        SideEffect::Expensive
    );
    assert_eq!(SideEffect::from_str("network").unwrap(), SideEffect::Network);
}

#[test]
fn from_str_rejects_unknown_tag() {
    let err = SideEffect::from_str("nope").unwrap_err();
    assert_eq!(
        err,
        ParseSideEffectError {
            input: "nope".to_string()
        }
    );
    let msg = format!("{err}");
    assert!(msg.contains("nope"));
}

#[test]
fn from_str_is_case_sensitive() {
    // We deliberately do not lowercase; "READ" should not match.
    assert!(SideEffect::from_str("READ").is_err());
    assert!(SideEffect::from_str("Read").is_err());
}

// ---- SideEffects set API ------------------------------------------------

#[test]
fn new_set_is_empty() {
    let s = SideEffects::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
}

#[test]
fn insert_remove_contains_len_iter() {
    let mut s = SideEffects::new();
    assert!(s.insert(SideEffect::Read));
    assert!(!s.insert(SideEffect::Read)); // already present
    assert!(s.insert(SideEffect::Network));
    assert_eq!(s.len(), 2);
    assert!(s.contains(SideEffect::Read));
    assert!(s.contains(SideEffect::Network));
    assert!(!s.contains(SideEffect::Write));

    let collected: Vec<SideEffect> = s.iter().copied().collect();
    assert_eq!(collected.len(), 2);
    assert!(collected.contains(&SideEffect::Read));
    assert!(collected.contains(&SideEffect::Network));

    assert!(s.remove(SideEffect::Read));
    assert!(!s.remove(SideEffect::Read)); // already gone
    assert!(!s.contains(SideEffect::Read));
    assert_eq!(s.len(), 1);
}

#[test]
fn from_iter_builds_set() {
    let s: SideEffects = [SideEffect::Write, SideEffect::Idempotent]
        .into_iter()
        .collect();
    assert_eq!(s.len(), 2);
    assert!(s.contains(SideEffect::Write));
    assert!(s.contains(SideEffect::Idempotent));
}

// ---- HasSideEffects trait impl example ----------------------------------

struct SearchWeb;
impl HasSideEffects for SearchWeb {
    fn side_effects(&self) -> SideEffects {
        let mut s = SideEffects::new();
        s.insert(SideEffect::Read);
        s.insert(SideEffect::Network);
        s
    }
}

struct DeleteAccount;
impl HasSideEffects for DeleteAccount {
    fn side_effects(&self) -> SideEffects {
        let mut s = SideEffects::new();
        s.insert(SideEffect::Destructive);
        s
    }
}

#[test]
fn has_side_effects_trait_can_be_implemented_on_user_types() {
    let s = SearchWeb.side_effects();
    assert!(s.contains(SideEffect::Read));
    assert!(s.contains(SideEffect::Network));
    let d = DeleteAccount.side_effects();
    assert!(d.contains(SideEffect::Destructive));
}

// ---- is_parallel_safe ---------------------------------------------------

#[test]
fn parallel_safe_when_read_only() {
    let s: SideEffects = [SideEffect::Read].into_iter().collect();
    assert!(is_parallel_safe(&s));
}

#[test]
fn parallel_safe_read_plus_network_is_ok() {
    let s: SideEffects = [SideEffect::Read, SideEffect::Network].into_iter().collect();
    assert!(is_parallel_safe(&s));
}

#[test]
fn not_parallel_safe_when_write_present() {
    let s: SideEffects = [SideEffect::Read, SideEffect::Write].into_iter().collect();
    assert!(!is_parallel_safe(&s));
}

#[test]
fn not_parallel_safe_when_destructive_present() {
    let s: SideEffects = [SideEffect::Read, SideEffect::Destructive]
        .into_iter()
        .collect();
    assert!(!is_parallel_safe(&s));
}

#[test]
fn not_parallel_safe_when_untagged() {
    let s = SideEffects::new();
    assert!(!is_parallel_safe(&s));
}

// ---- is_retry_safe ------------------------------------------------------

#[test]
fn retry_safe_when_idempotent() {
    let s: SideEffects = [SideEffect::Write, SideEffect::Idempotent]
        .into_iter()
        .collect();
    assert!(is_retry_safe(&s));
}

#[test]
fn retry_safe_when_read_only() {
    let s: SideEffects = [SideEffect::Read].into_iter().collect();
    assert!(is_retry_safe(&s));
}

#[test]
fn not_retry_safe_when_destructive_even_with_idempotent() {
    let s: SideEffects = [SideEffect::Destructive, SideEffect::Idempotent]
        .into_iter()
        .collect();
    assert!(!is_retry_safe(&s));
}

#[test]
fn not_retry_safe_external_without_idempotent() {
    let s: SideEffects = [SideEffect::External].into_iter().collect();
    assert!(!is_retry_safe(&s));
}

#[test]
fn retry_safe_external_with_idempotent() {
    let s: SideEffects = [SideEffect::External, SideEffect::Idempotent]
        .into_iter()
        .collect();
    assert!(is_retry_safe(&s));
}

#[test]
fn not_retry_safe_when_untagged() {
    assert!(!is_retry_safe(&SideEffects::new()));
}

#[test]
fn not_retry_safe_when_read_and_write_both_present() {
    let s: SideEffects = [SideEffect::Read, SideEffect::Write].into_iter().collect();
    assert!(!is_retry_safe(&s));
}

// ---- is_destructive -----------------------------------------------------

#[test]
fn is_destructive_true_when_tagged() {
    let s: SideEffects = [SideEffect::Destructive].into_iter().collect();
    assert!(is_destructive(&s));
}

#[test]
fn is_destructive_false_otherwise() {
    let s: SideEffects = [SideEffect::Write].into_iter().collect();
    assert!(!is_destructive(&s));
    assert!(!is_destructive(&SideEffects::new()));
}

// ---- Tag<T> -------------------------------------------------------------

#[test]
fn tag_new_value_effects_into_inner() {
    let mut effects = SideEffects::new();
    effects.insert(SideEffect::Write);
    effects.insert(SideEffect::Idempotent);
    let tag = Tag::new(String::from("upsert_user"), effects);

    assert_eq!(tag.value(), "upsert_user");
    assert!(tag.effects().contains(SideEffect::Write));
    assert!(tag.effects().contains(SideEffect::Idempotent));

    let inner = tag.into_inner();
    assert_eq!(inner, "upsert_user");
}

#[derive(Debug, PartialEq)]
struct ToolHandle {
    name: &'static str,
    arity: usize,
}

#[test]
fn tag_can_wrap_any_struct() {
    let handle = ToolHandle {
        name: "delete_account",
        arity: 1,
    };
    let mut effects = SideEffects::new();
    effects.insert(SideEffect::Destructive);
    let tag = Tag::new(handle, effects);

    assert_eq!(tag.value().name, "delete_account");
    assert_eq!(tag.value().arity, 1);
    assert!(is_destructive(tag.effects()));

    // The Tag itself satisfies HasSideEffects.
    let from_trait = tag.side_effects();
    assert!(from_trait.contains(SideEffect::Destructive));
}

// ---- serde (only when the feature is on) -------------------------------

#[cfg(feature = "serde")]
#[test]
fn serde_roundtrip_single_effect() {
    let s = serde_json::to_string(&SideEffect::Idempotent).unwrap();
    assert_eq!(s, "\"idempotent\"");
    let back: SideEffect = serde_json::from_str(&s).unwrap();
    assert_eq!(back, SideEffect::Idempotent);
}

#[cfg(feature = "serde")]
#[test]
fn serde_roundtrip_set() {
    let mut effects = SideEffects::new();
    effects.insert(SideEffect::Read);
    effects.insert(SideEffect::Network);
    let json = serde_json::to_string(&effects).unwrap();
    let back: SideEffects = serde_json::from_str(&json).unwrap();
    assert_eq!(back, effects);
}
