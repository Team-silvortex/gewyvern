# Leserpent Mobile

`Leserpent.MobileCore` is the host-independent mobile lifecycle boundary for
the Leserpent 2 remote console. It does not depend on Android or iOS workloads,
so its security and reentry semantics remain testable on every development
host.

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
fallback. Platform application entry projects and physical-device AOT evidence
remain the next slice.
