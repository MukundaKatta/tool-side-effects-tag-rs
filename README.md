# tool-side-effects-tag

[![Crates.io](https://img.shields.io/crates/v/tool-side-effects-tag.svg)](https://crates.io/crates/tool-side-effects-tag)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**Declare what an LLM agent tool actually does** so the scheduler / retry layer can make the right decision per-tool. Zero runtime deps.

```rust
use tool_side_effects_tag::{
    is_destructive, is_parallel_safe, is_retry_safe,
    HasSideEffects, SideEffect, SideEffects,
};

struct SearchWeb;
impl HasSideEffects for SearchWeb {
    fn side_effects(&self) -> SideEffects {
        let mut s = SideEffects::new();
        s.insert(SideEffect::Read);
        s.insert(SideEffect::Network);
        s
    }
}

struct UpsertUser;
impl HasSideEffects for UpsertUser {
    fn side_effects(&self) -> SideEffects {
        let mut s = SideEffects::new();
        s.insert(SideEffect::Write);
        s.insert(SideEffect::Idempotent);
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

let search = SearchWeb.side_effects();
let upsert = UpsertUser.side_effects();
let delete = DeleteAccount.side_effects();

assert!(is_parallel_safe(&search));   // pure read
assert!(!is_parallel_safe(&upsert));  // writes
assert!(is_retry_safe(&upsert));      // idempotent write
assert!(is_destructive(&delete));
```

## Why

Most agent loops treat every tool the same way. If retry is on, it's on for everything, including `send_email`, which is unfortunate when the network blip causes a duplicate. If parallelism is on, it's on for everything, including `upsert_user`, which races with itself.

`tool-side-effects-tag` is a one-line declaration so the dispatcher knows what to do per-tool. Zero magic.

The standard tags:

| Tag | What it means |
|---|---|
| `Read` | No state mutation. Safe to parallelize and retry. |
| `Write` | Mutates state. Not parallel-safe by default. |
| `Idempotent` | Same args produce the same effect. Retry-safe. |
| `Destructive` | Delete/drop/purge. Never auto-retry. |
| `External` | Third-party system (email, payments). Not retry-safe without `Idempotent`. |
| `Expensive` | High cost. Caller may want extra confirmation. |
| `Network` | Makes a network call. Subject to transient errors. |

## Install

```toml
[dependencies]
tool-side-effects-tag = "0.1"
```

Optional serde support:

```toml
[dependencies]
tool-side-effects-tag = { version = "0.1", features = ["serde"] }
```

## API

```rust
use tool_side_effects_tag::{
    HasSideEffects,     // implement on your tool types
    SideEffect,         // the enum
    SideEffects,        // a set of tags
    Tag,                // wrap any T with a SideEffects set
    is_parallel_safe,   // &SideEffects -> bool
    is_retry_safe,      // &SideEffects -> bool
    is_destructive,     // &SideEffects -> bool
};
```

### `SideEffect`

`Debug + Clone + Copy + PartialEq + Eq + Hash`, plus `Display` and `FromStr` over the lowercase slugs `"read"`, `"write"`, `"idempotent"`, `"destructive"`, `"external"`, `"expensive"`, `"network"`. Unknown inputs return `ParseSideEffectError`.

### `SideEffects`

Thin wrapper around `HashSet<SideEffect>`:

```rust
use tool_side_effects_tag::{SideEffect, SideEffects};

let mut s = SideEffects::new();
s.insert(SideEffect::Read);
s.insert(SideEffect::Network);
assert!(s.contains(SideEffect::Read));
assert_eq!(s.len(), 2);
s.remove(SideEffect::Network);
for e in s.iter() {
    println!("{e}");
}
```

`SideEffects` also implements `FromIterator<SideEffect>` so you can do:

```rust
use tool_side_effects_tag::{SideEffect, SideEffects};
let s: SideEffects = [SideEffect::Write, SideEffect::Idempotent].into_iter().collect();
```

### `Tag<T>`

Pair any value with a `SideEffects` set:

```rust
use tool_side_effects_tag::{SideEffect, SideEffects, Tag};

let mut effects = SideEffects::new();
effects.insert(SideEffect::Destructive);
let tag = Tag::new("delete_account", effects);

assert_eq!(*tag.value(), "delete_account");
assert!(tag.effects().contains(SideEffect::Destructive));
let inner = tag.into_inner();
```

`Tag<T>` itself implements `HasSideEffects`, so you can pass a `Tag<MyTool>` anywhere a `HasSideEffects` is expected.

## Conservative defaults

- An **untagged** (empty) set returns `false` for both `is_parallel_safe` and `is_retry_safe`. If you don't know what a tool does, don't run it in parallel and don't retry it.
- A **destructive** set returns `false` for `is_retry_safe` even if also tagged `Idempotent`. Destructive intent overrides idempotent for retry purposes. If you actually want the retry, the caller should ask for it explicitly.

## Companion libraries

- [`llm-retry`](https://github.com/MukundaKatta/llm-retry) — gate retries on `is_retry_safe(tool)`.
- [`agentleash`](https://github.com/MukundaKatta/agentleash) — gate destructive calls on operator confirmation.
- [`tool-side-effects-tag`](https://github.com/MukundaKatta/tool-side-effects-tag) — the Python sibling.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
