# Reference: Protocol Alias Policy

This page defines the alias rules that built-in fallback and registry-backed
resolution are expected to follow together.

## Scope

- protocol aliases may repeat across different protocol families
- protocol aliases must not point to multiple targets inside the same family
- entry aliases may repeat across different protocol families
- entry aliases must not shadow a different canonical entry inside the same family

## Style

- aliases stay lowercase
- aliases use only ASCII letters, digits, `-`, and `_`
- prefixed protocol aliases should keep dash and snake variants together when
  both forms are meaningful, such as `redis-get` and `redis_get`

## Intent

- protocol aliases are for family-level shortcuts and packaged entry handles
- entry aliases are for operator vocabulary such as `login`, `query`, `send`,
  `listen`, `directory`, or `replication`
- built-in fallback should preserve the same high-value aliases as the packaged
  registry whenever possible

## Enforcement

- [src/protocol_profiles/tests_alias_policy.rs](/Users/Shared/chroot/dev/gewyvern/src/protocol_profiles/tests_alias_policy.rs)
- [src/protocol_profiles/tests_fallback.rs](/Users/Shared/chroot/dev/gewyvern/src/protocol_profiles/tests_fallback.rs)
- [src/protocol_profiles/tests_manifest_parity.rs](/Users/Shared/chroot/dev/gewyvern/src/protocol_profiles/tests_manifest_parity.rs)
