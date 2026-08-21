use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn ios_entry_is_native_secure_and_uses_shared_mobile_policy() {
    let root = "apps/leserpent-mobile/src/Leserpent.Mobile.iOS";
    let project = source(&format!("{root}/Leserpent.Mobile.iOS.csproj"));
    let plist = source(&format!("{root}/Info.plist"));
    let app_delegate = source(&format!("{root}/AppDelegate.cs"));
    let scene = source(&format!("{root}/SceneDelegate.cs"));
    let hub = source(&format!("{root}/MobileHubViewController.cs"));
    let renderer = source(&format!("{root}/IosUiDocumentView.cs"));
    let keychain = source(&format!("{root}/IosKeychainSecretStore.cs"));
    let profile = source(&format!("{root}/IosConnectionProfileStore.cs"));
    let platform_proof = source(&format!("{root}/IosPlatformProof.cs"));

    for required in [
        "<OutputType>Exe</OutputType>",
        "<TargetFramework>net10.0-ios</TargetFramework>",
        "<ApplicationId>org.gewyvern.leserpent</ApplicationId>",
        "<ApplicationDisplayVersion>$(Version)</ApplicationDisplayVersion>",
        "<SupportedOSPlatformVersion>15.0</SupportedOSPlatformVersion>",
        "<RuntimeIdentifier Condition=\"'$(RuntimeIdentifier)' == ''\">iossimulator-arm64</RuntimeIdentifier>",
        "<TrimMode Condition=\"'$(Configuration)' == 'Release'\">full</TrimMode>",
    ] {
        assert!(project.contains(required), "iOS project lost {required}");
    }
    assert!(
        !project.contains("ValidateXcodeVersion"),
        "iOS project disabled the supported Xcode version fence"
    );
    for required in [
        "<string>org.gewyvern.leserpent</string>",
        "<key>UIApplicationSceneManifest</key>",
        "<key>NSLocalNetworkUsageDescription</key>",
        "<string>Dark</string>",
        "Assets.xcassets/AppIcon.appiconset",
    ] {
        assert!(plist.contains(required), "iOS metadata lost {required}");
    }
    assert!(
        !plist.contains("NSAllowsArbitraryLoads")
            && !plist.contains("NSExceptionAllowsInsecureHTTPLoads"),
        "iOS metadata weakened App Transport Security"
    );

    for required in [
        "new MobileCredentialVault(new IosKeychainSecretStore())",
        "hub ??= new MobileHubViewController(",
        "await hub.EnterForegroundAsync()",
        "await hub.EnterBackgroundAsync()",
        "ShowPrivacyShield()",
        "HidePrivacyShield()",
        "sceneWillResignActive:",
        "sceneDidEnterBackground:",
    ] {
        assert!(
            scene.contains(required),
            "iOS scene lifecycle lost {required}"
        );
    }
    assert!(
        !scene.contains("await hub.DisposeAsync()"),
        "transient iOS scene disconnect retired the reusable application coordinator"
    );
    assert!(
        scene.find("HidePrivacyShield()").unwrap()
            < scene.find("await hub.EnterForegroundAsync()").unwrap(),
        "active iOS UI remained hidden behind a network-dependent foreground transition"
    );

    for required in [
        "new MobileApplicationCoordinator(vault)",
        "--leserpent-ui-proof",
        "MobileLayoutPolicy.Resolve(",
        "View.KeyboardLayoutGuide.TopAnchor",
        "MobileUiDocumentBinding.Project(",
        "RemoteDocumentProjection.Project(feed).Document",
        "RemoteWorkspaceDocumentProjection.Project(workspace)",
        "source.ResolveSubmission(node.Id, values, currentFeed)",
        "coordinator.LoadWorkspaceAsync(runtime.Id, lifetime.Token)",
        "coordinator.ExecuteMutationAsync(intent, lifetime.Token)",
        "ShowParameterizedFormAsync(form)",
        "ShowConfirmationAsync(",
        "catch (OperationCanceledException) when (lifetime.IsCancellationRequested)",
        "button.TitleLabel.Lines = 0",
        "Workspace network request failed safely.",
        "Remote change failed safely.",
    ] {
        assert!(hub.contains(required), "iOS native hub lost {required}");
    }
    assert!(
        !hub.contains("new RemoteWorkspaceClient")
            && !hub.contains("new RemoteMutationClient")
            && !hub.contains("Feed.Runtimes"),
        "iOS native hub crossed the shared mobile policy boundary"
    );

    for required in [
        "MobileUiDocumentBinding document",
        "MobileUiNodeBinding node",
        "AccessibilityIdentifier = node.Id",
        "ActionEnabled(node.ActionKind, availability)",
        "await invokeAction(document, node)",
        "No runtime projection available.",
    ] {
        assert!(
            renderer.contains(required),
            "iOS document renderer lost {required}"
        );
    }
    assert!(
        !renderer.contains("RemoteFeedState") && !renderer.contains("RemoteWorkspaceClient"),
        "iOS document renderer introduced transport or feed ownership"
    );

    for required in [
        "SecKind.GenericPassword",
        "SecAccessible.WhenUnlockedThisDeviceOnly",
        "SecKeyChain.Update",
        "SecKeyChain.Add",
        "SecKeyChain.Remove",
    ] {
        assert!(
            keychain.contains(required),
            "iOS Keychain boundary lost {required}"
        );
    }
    for required in [
        "new MobileConnectionProfileStore(",
        "NSUserDefaults.StandardUserDefaults",
        "NSSearchPathDirectory.ApplicationSupportDirectory",
        "NSSearchPathDirectory.CachesDirectory",
    ] {
        assert!(
            profile.contains(required),
            "iOS profile bridge lost {required}"
        );
    }
    assert!(
        !profile.contains("token") && !profile.contains("secret"),
        "iOS endpoint profile persisted credential material"
    );

    for required in [
        "#if DEBUG",
        "--leserpent-keychain-proof",
        "MobileCredentialVault(new IosKeychainSecretStore())",
        "await vault.StoreAsync",
        "await vault.LoadAsync",
        "await vault.DeleteAsync",
        "sensitive_values_retained\\\":false",
    ] {
        assert!(
            platform_proof.contains(required)
                || project.contains(required)
                || app_delegate.contains(required),
            "iOS platform proof lost {required}"
        );
    }
    assert!(
        !platform_proof.contains("error.Message") && !platform_proof.contains("token\":"),
        "iOS platform proof persisted sensitive diagnostics"
    );
}
