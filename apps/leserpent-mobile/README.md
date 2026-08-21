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

Connection metadata follows the same boundary. `MobileConnectionProfileStore`
canonicalizes HTTPS endpoints, hashes endpoint-derived CA/cache filenames,
rejects malformed certificates and private keys, and replaces public CA files
atomically in app-private storage. Android and iOS persist only the canonical
endpoint through their native preferences adapters; tokens never enter the
profile, and malformed or unavailable stored state fails closed.

The Android project is now an executable entry client. Its native `MainActivity`
accepts an HTTPS authority, public CA certificate, and endpoint-scoped token;
only the endpoint is stored in private preferences, the CA is validated and
written to app-private files, and the token goes exclusively through Android
Keystore. `MobileApplicationCoordinator` maps duplicate platform start/stop
callbacks onto one foreground session and generation-fenced background
disconnect. The runtime view is the default working surface; saved connection
setup collapses after onboarding and reopens explicitly without placing secrets
in the summary. Fleet and workspace controls now come from the shared
`UiDocument` projections through `MobileUiDocumentBinding`, which validates and
clones the semantic source before exposing immutable native-control metadata.
Inspect loads a generation-fenced workspace, while refresh, capability discovery,
and deployment route through the shared typed action and mutation coordinators.
Deployment fields are created from the capability-gated shared form, become a
validated `submit` event, and require a second native confirmation before the
transport is admitted. Form values are never added to the document or profile.

The iOS project is also an executable native entry client. Its UIKit scene
composes the same application coordinator, shared connection profile, layout
policy, fleet/workspace documents, typed form events, and mutation coordinator.
The platform layer supplies native controls, safe-area and Dynamic Type inputs,
keyboard avoidance, scene foreground/background callbacks, a task-switcher
privacy shield, and Keychain storage. It does not own transport projections or
read feed runtimes directly. The branded launch screen and app icon are bundled
through the asset catalog.

`MobileLayoutPolicy` keeps adaptive behavior outside either native host. It
classifies the safe, font-scaled viewport as Compact (below 600 dp), Medium, or
Expanded (840 dp and above), keeps touch targets at least 48 dp, bounds wide
content, falls back from two panes in short landscape windows, and selects one
or two runtime-card columns. The Android host projects the plan into native
controls, accounts for system bars and display cutouts under edge-to-edge
rendering, and keeps the setup action above the on-screen keyboard in a bottom
action area.
Both native hosts consume this plan. Extremely narrow multi-window surfaces and
oversized accessibility text
degrade to one column rather than rejecting a valid platform window. The
resolved plan is a value type, and IME-only changes update action padding
without rebuilding structural layout parameters.

Host-independent conformance plus `tests/android_entry_contract_tdd.rs` and
`tests/ios_entry_contract_tdd.rs` validate the native compositions, immutable
document binding, form-event route, mutation fence, adaptive policy, and secure
platform-storage boundaries without loading an emulator. The locked Android
proof additionally builds a directly installable APK and dual-ABI AOT AAB with
.NET SDK 10.0.201, Android workload 36.1.2, API 36, and Microsoft OpenJDK 17.
It exercises Compact, Medium, Expanded, short-landscape, 1.5x font, display
cutout, IME, cold-start, and hot-resume behavior on an API 36 ARM64 emulator.
The retained result is
`docs/fixtures/leserpent_android_api36_emulator_macos_arm64_20260821.json`.

With `ANDROID_SDK_ROOT` and `JAVA_HOME` set, reproduce the package builds with:

```bash
dotnet build \
  apps/leserpent-mobile/src/Leserpent.Mobile.Android/Leserpent.Mobile.Android.csproj \
  -c Debug -r android-arm64 -p:StandaloneAndroidPackage=true \
  -p:AndroidSdkDirectory="$ANDROID_SDK_ROOT" -p:JavaSdkDirectory="$JAVA_HOME"

dotnet build \
  apps/leserpent-mobile/src/Leserpent.Mobile.Android/Leserpent.Mobile.Android.csproj \
  -c Release -p:AndroidSdkDirectory="$ANDROID_SDK_ROOT" \
  -p:JavaSdkDirectory="$JAVA_HOME"
```

On macOS with Xcode 26.5, use .NET SDK 10.0.300 and iOS workload set
10.0.300.2 (iOS workload 26.5.10280). Build a simulator app and an unsigned
device-shaped release bundle with:

```bash
dotnet workload install ios --version 10.0.300.2
xcodebuild -downloadPlatform iOS -architectureVariant arm64

dotnet build \
  apps/leserpent-mobile/src/Leserpent.Mobile.iOS/Leserpent.Mobile.iOS.csproj \
  -c Debug -r iossimulator-arm64

dotnet build \
  apps/leserpent-mobile/src/Leserpent.Mobile.iOS/Leserpent.Mobile.iOS.csproj \
  -c Release -r ios-arm64 -p:EnableCodeSigning=false
```

Ordinary Debug builds retain fast deployment. Visual QA may explicitly set
`LeserpentUiCapture=true`; the project accepts that switch only in Debug, while
all production-shaped builds retain `FLAG_SECURE`. The next Android gate is
production signing and physical-device safe-area, font-scale, Keystore, and TLS
proof. The remaining iOS release gates are production Apple signing and
physical-device safe-area, Dynamic Type, Keychain, TLS, and lifecycle proof.
