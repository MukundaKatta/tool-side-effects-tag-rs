//! # tool-side-effects-tag
//!
//! Declare what an LLM agent tool actually does so the scheduler / retry
//! layer can make the right decision per-tool.
//!
//! If your scheduler does not know which tools are reads and which are
//! writes, it cannot run them in parallel safely. If your retry layer does
//! not know which tools are idempotent, it cannot retry them safely. This
//! crate gives you a tiny vocabulary of [`SideEffect`] tags plus inspection
//! helpers that classify a tool as parallel-safe, retry-safe, or
//! destructive.
//!
//! The Python sibling uses a function decorator. Rust does not have
//! decorators, so the idiom here is an associated-metadata pattern: tools
//! implement [`HasSideEffects`] (or hold a [`Tag`] / [`SideEffects`]) and
//! the free functions [`is_parallel_safe`], [`is_retry_safe`], and
//! [`is_destructive`] read that set.
//!
//! ## Quick example
//!
//! ```
//! use tool_side_effects_tag::{
//!     HasSideEffects, SideEffect, SideEffects,
//!     is_destructive, is_parallel_safe, is_retry_safe,
//! };
//!
//! struct SearchWeb;
//! impl HasSideEffects for SearchWeb {
//!     fn side_effects(&self) -> SideEffects {
//!         let mut s = SideEffects::new();
//!         s.insert(SideEffect::Read);
//!         s
//!     }
//! }
//!
//! let tool = SearchWeb;
//! assert!(is_parallel_safe(&tool.side_effects()));
//! assert!(is_retry_safe(&tool.side_effects()));
//! assert!(!is_destructive(&tool.side_effects()));
//! ```
//!
//! ## With a [`Tag`] wrapper
//!
//! ```
//! use tool_side_effects_tag::{SideEffect, SideEffects, Tag};
//!
//! let mut effects = SideEffects::new();
//! effects.insert(SideEffect::Write);
//! effects.insert(SideEffect::Idempotent);
//! let upsert = Tag::new("upsert_user", effects);
//!
//! assert_eq!(*upsert.value(), "upsert_user");
//! assert!(upsert.effects().contains(SideEffect::Write));
//! ```
//!
//! ## Feature flags
//!
//! - `serde` — derives `Serialize` / `Deserialize` for [`SideEffect`] and
//!   [`SideEffects`]. Off by default; enable with
//!   `features = ["serde"]`.

#![deny(missing_docs)]

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Standard side-effect categories for an agent tool.
///
/// String form matches the Python sibling library:
/// `"read"`, `"write"`, `"idempotent"`, `"destructive"`, `"external"`,
/// `"expensive"`, `"network"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum SideEffect {
    /// Reads data. No state mutation. Safe to parallelize and retry.
    Read,
    /// Mutates internal state. Not parallel-safe by default.
    Write,
    /// Repeated calls with the same args produce the same effect. Retry-safe.
    Idempotent,
    /// Removes or invalidates state (delete, drop, purge). Never auto-retry.
    Destructive,
    /// Touches a third-party system (email, payments, webhooks). Not
    /// retry-safe without [`SideEffect::Idempotent`].
    External,
    /// High cost (tokens, money, time). Caller may want extra confirmation.
    Expensive,
    /// Makes a network call. Subject to retryable transient errors.
    Network,
}

impl SideEffect {
    /// Stable string slug used by [`Display`](fmt::Display) and [`FromStr`].
    pub fn as_str(&self) -> &'static str {
        match self {
            SideEffect::Read => "read",
            SideEffect::Write => "write",
            SideEffect::Idempotent => "idempotent",
            SideEffect::Destructive => "destructive",
            SideEffect::External => "external",
            SideEffect::Expensive => "expensive",
            SideEffect::Network => "network",
        }
    }
}

impl fmt::Display for SideEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned by [`SideEffect::from_str`] when the input does not match
/// any known tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSideEffectError {
    /// The input that failed to parse.
    pub input: String,
}

impl fmt::Display for ParseSideEffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown side-effect tag: {:?}", self.input)
    }
}

impl std::error::Error for ParseSideEffectError {}

impl FromStr for SideEffect {
    type Err = ParseSideEffectError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read" => Ok(SideEffect::Read),
            "write" => Ok(SideEffect::Write),
            "idempotent" => Ok(SideEffect::Idempotent),
            "destructive" => Ok(SideEffect::Destructive),
            "external" => Ok(SideEffect::External),
            "expensive" => Ok(SideEffect::Expensive),
            "network" => Ok(SideEffect::Network),
            other => Err(ParseSideEffectError {
                input: other.to_string(),
            }),
        }
    }
}

/// An unordered set of [`SideEffect`] tags attached to a tool.
///
/// Backed by [`HashSet`]; insertion order is not preserved. Use
/// [`SideEffects::iter`] to read out the contents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct SideEffects {
    inner: HashSet<SideEffect>,
}

impl SideEffects {
    /// Construct an empty set.
    pub fn new() -> Self {
        Self {
            inner: HashSet::new(),
        }
    }

    /// Insert a tag. Returns `true` if the tag was newly added.
    pub fn insert(&mut self, effect: SideEffect) -> bool {
        self.inner.insert(effect)
    }

    /// Remove a tag. Returns `true` if the tag was present.
    pub fn remove(&mut self, effect: SideEffect) -> bool {
        self.inner.remove(&effect)
    }

    /// Returns `true` iff the set contains `effect`.
    pub fn contains(&self, effect: SideEffect) -> bool {
        self.inner.contains(&effect)
    }

    /// Iterate over the tags in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = &SideEffect> {
        self.inner.iter()
    }

    /// Number of tags in the set.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` iff the set is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl FromIterator<SideEffect> for SideEffects {
    fn from_iter<I: IntoIterator<Item = SideEffect>>(iter: I) -> Self {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

/// Implement on a tool type to declare its side-effect surface.
///
/// The dispatch / retry layer can then call [`is_parallel_safe`],
/// [`is_retry_safe`], or [`is_destructive`] on the returned set without
/// caring what the concrete tool type is.
pub trait HasSideEffects {
    /// Return the side-effect set for this tool.
    fn side_effects(&self) -> SideEffects;
}

/// Pairs any value with a [`SideEffects`] set. Lets you tag tool handles,
/// closures, command structs, anything.
#[derive(Debug, Clone)]
pub struct Tag<T> {
    value: T,
    effects: SideEffects,
}

impl<T> Tag<T> {
    /// Wrap `value` with the given `effects` set.
    pub fn new(value: T, effects: SideEffects) -> Self {
        Self { value, effects }
    }

    /// Borrow the inner value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Borrow the attached side-effect set.
    pub fn effects(&self) -> &SideEffects {
        &self.effects
    }

    /// Consume the [`Tag`] and return the inner value, dropping the tags.
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> HasSideEffects for Tag<T> {
    fn side_effects(&self) -> SideEffects {
        self.effects.clone()
    }
}

/// Safe to run alongside other tools.
///
/// Rules:
/// - [`SideEffect::Read`]-only with no [`SideEffect::Write`] / [`SideEffect::Destructive`] -> safe.
/// - [`SideEffect::Write`] / [`SideEffect::Destructive`] present -> not safe.
/// - Empty / untagged -> not safe (conservative default).
pub fn is_parallel_safe(effects: &SideEffects) -> bool {
    if effects.is_empty() {
        return false;
    }
    if effects.contains(SideEffect::Write) || effects.contains(SideEffect::Destructive) {
        return false;
    }
    effects.contains(SideEffect::Read)
}

/// Safe to auto-retry on transient error.
///
/// Rules:
/// - [`SideEffect::Destructive`] -> never (caller must opt in per-call).
/// - [`SideEffect::Idempotent`] explicitly tagged -> safe.
/// - [`SideEffect::Read`]-only with no [`SideEffect::Write`] -> safe.
/// - Otherwise -> not safe.
pub fn is_retry_safe(effects: &SideEffects) -> bool {
    if effects.is_empty() {
        return false;
    }
    if effects.contains(SideEffect::Destructive) {
        return false;
    }
    if effects.contains(SideEffect::Idempotent) {
        return true;
    }
    if effects.contains(SideEffect::Read) && !effects.contains(SideEffect::Write) {
        return true;
    }
    false
}

/// `true` iff the set contains [`SideEffect::Destructive`].
pub fn is_destructive(effects: &SideEffects) -> bool {
    effects.contains(SideEffect::Destructive)
}
