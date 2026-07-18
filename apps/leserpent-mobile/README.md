# Leserpent Mobile

`Leserpent.MobileCore` is the host-independent mobile lifecycle boundary for
the Leserpent 2 remote console. It does not depend on Android or iOS workloads,
so its security and reentry semantics remain testable on every development
host.

MobileCore also consumes the renderer-neutral fleet and runtime-workspace
`UiDocument` projections from `Leserpent.RemoteClient`. The deterministic
conformance runner verifies filtering, endpoint isolation, capability-gated
deployment forms, and the shared workspace policies without loading Avalonia;
native shells therefore supply controls and navigation rather than a second
business projection.
The shared client also owns the post-mutation revision and observation fences.
After an unknown network outcome, neither desktop nor mobile may issue another
mutation until a newer authoritative snapshot satisfies the same runtime and
capability-observation rules; heartbeats alone never release the fence.
Mutation and inspection availability are projected by the same shared policy.
It gives in-flight work precedence over revision and observation fences,
disables both action classes when state is stale, and returns bounded reasons
for native controls to present without reimplementing authorization state.
Authority health and effect-queue pressure use a shared presentation contract as
well. Ready, nominal queue, and saturated queue states expose the same bounded
label, automation description, and severity flag on every host.

The lifecycle owns these rules:

- a remote session exists only while the application is foregrounded;
- entering background invalidates the session generation before disposal and
  presents retained projection data as stale;
- every foreground reentry reloads the endpoint-scoped token from the injected
  mobile credential vault;
- endpoint-bound cached projection state is published as stale before a live
  foreground snapshot replaces it;
- events from retired sessions cannot cross the generation fence;
- missing or malformed credentials fail before a transport is created;
- startup failure and terminal disposal release sessions exactly once.

Run the deterministic contract:

```bash
dotnet run --project \
  apps/leserpent-mobile/src/Leserpent.MobileConformance/Leserpent.MobileConformance.csproj
```

The platform credential adapters are intentionally narrow:

- `Leserpent.Mobile.Android` encrypts each token with AES-256-GCM, keeps the
  master key in Android Keystore, and stores only the authenticated envelope in
  private preferences;
- `Leserpent.Mobile.iOS` stores tokens as generic-password Keychain items with
  `WhenUnlockedThisDeviceOnly` accessibility.

Both implement `IMobileSecretStore` behind the validating
`MobileCredentialVault`. The shared adapter hashes endpoint aliases, validates
before write and after read, and fences cancelled operations before they reach
platform storage. Hosts then delegate foreground/background callbacks to
`MobileRemoteLifecycle`; they must not use the desktop environment-token
fallback.

The Android project is now an executable entry client. Its native `MainActivity`
accepts an HTTPS authority, public CA certificate, and endpoint-scoped token;
only the endpoint is stored in private preferences, the CA is validated and
written to app-private files, and the token goes exclusively through Android
Keystore. `MobileApplicationCoordinator` maps duplicate platform start/stop
callbacks onto one foreground session and generation-fenced background
disconnect. The initial shell renders connection state and bounded runtime
summaries without introducing Android-owned command semantics.

Host-independent conformance and `tests/android_entry_contract_tdd.rs` validate
the composition without an Android SDK. The next Android gate is a locked
workload build, emulator launch, physical-device Keystore/TLS proof, and reuse
of the renderer-neutral parameterized form-event contract. iOS follows only
after that Android parity is stable.
