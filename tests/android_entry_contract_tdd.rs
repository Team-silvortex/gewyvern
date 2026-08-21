use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn android_entry_client_preserves_the_mobile_security_boundary() {
    let project = source(
        "apps/leserpent-mobile/src/Leserpent.Mobile.Android/Leserpent.Mobile.Android.csproj",
    );
    let activity = source("apps/leserpent-mobile/src/Leserpent.Mobile.Android/MainActivity.cs");
    let profile = source(
        "apps/leserpent-mobile/src/Leserpent.Mobile.Android/AndroidConnectionProfileStore.cs",
    );
    let shared_profile =
        source("apps/leserpent-mobile/src/Leserpent.MobileCore/MobileConnectionProfileStore.cs");

    for required in [
        "<OutputType>Exe</OutputType>",
        "<TargetFramework>net10.0-android</TargetFramework>",
        "<ApplicationId>org.gewyvern.leserpent</ApplicationId>",
        "<RuntimeIdentifiers Condition=\"'$(RuntimeIdentifier)' == ''\">android-arm64;android-x64</RuntimeIdentifiers>",
        "<UseDefaultPublishRuntimeIdentifier>false</UseDefaultPublishRuntimeIdentifier>",
        "<EmbedAssembliesIntoApk Condition=\"'$(StandaloneAndroidPackage)' == 'true' or '$(LeserpentUiCapture)' == 'true'\">true</EmbedAssembliesIntoApk>",
        "<AndroidPackageFormat Condition=\"'$(Configuration)' == 'Release'\">aab</AndroidPackageFormat>",
        "LogicalName=\"Resources/mipmap/leserpent_icon.png\"",
        "Condition=\"'$(LeserpentUiCapture)' == 'true' and '$(Configuration)' != 'Debug'\"",
    ] {
        assert!(
            project.contains(required),
            "Android project lost {required}"
        );
    }
    for required in [
        "MainLauncher = true",
        "Icon = \"@mipmap/leserpent_icon\"",
        "UsesPermission(Android.Manifest.Permission.Internet)",
        "new AndroidKeystoreSecretStore(this)",
        "new MobileApplicationCoordinator(",
        "WindowManagerFlags.Secure",
        "#if !DEBUG || !LESERPENT_UI_CAPTURE",
        "coordinator.EnterForegroundAsync",
        "coordinator.EnterBackgroundAsync",
        "MobileCredentialVault",
    ] {
        assert!(activity.contains(required), "Android entry lost {required}");
    }
    assert!(
        !activity.contains("GetSharedPreferences")
            && !activity.contains("LESERPENT_REMOTE_TOKEN")
            && !activity.contains("Intent.GetStringExtra"),
        "Android activity introduced an untrusted credential source"
    );
    for required in [
        "FileCreationMode.Private",
        "applicationContext.FilesDir",
        "new MobileConnectionProfileStore(",
        "AndroidEndpointStore",
    ] {
        assert!(
            profile.contains(required),
            "Android profile lost {required}"
        );
    }
    assert!(
        !profile.contains("token") && !profile.contains("secret"),
        "Android profile store must never persist credentials"
    );
    for required in [
        "public interface IMobileEndpointStore",
        "RemoteClientOptions.ParseEndpoint",
        "X509Certificate2.CreateFromPem",
        "PRIVATE KEY",
        "FileOptions.WriteThrough",
        "stream.Flush(flushToDisk: true)",
        "EndpointDigest(endpoint)",
    ] {
        assert!(
            shared_profile.contains(required),
            "shared mobile profile store lost {required}"
        );
    }
}

#[test]
fn android_entry_uses_shared_safe_adaptive_layout() {
    let activity = source("apps/leserpent-mobile/src/Leserpent.Mobile.Android/MainActivity.cs");
    let policy = source("apps/leserpent-mobile/src/Leserpent.MobileCore/MobileLayoutPolicy.cs");
    let conformance = source("apps/leserpent-mobile/src/Leserpent.MobileConformance/Program.cs");

    for required in [
        "MobileLayoutPolicy.Resolve(",
        "SetDecorFitsSystemWindows(false)",
        "OnApplyWindowInsets",
        "WindowInsets.Type.SystemBars()",
        "WindowInsets.Type.DisplayCutout()",
        "WindowInsets.Type.Ime()",
        "!OperatingSystem.IsAndroidVersionAtLeast(35)",
        "new AdaptiveRootLayout(this, UpdateWindowInsets)",
        "rootLayout.RequestApplyInsets()",
        "Math.Ceiling(imeBottomDp)",
        "appliedLayoutPlan == plan",
        "SetConnectionExpanded(profile is null)",
        "actionBar.Visibility",
        "plan.TwoPane",
        "plan.RuntimeColumns",
        "plan.ContentMaxWidthDp",
        "plan.MinimumTouchTargetDp",
        "Input(\"Keystore protected\")",
        "SetConnectEnabled(false)",
        "enabled ? \"#FFB229\" : \"#5A5142\"",
        "RuntimeColumns()",
        "ConfigChanges.SmallestScreenSize",
    ] {
        assert!(
            activity.contains(required),
            "Android layout lost {required}"
        );
    }
    assert!(activity
        .contains("operationFailed || snapshot.Error is not null ? \"#FF8A65\" : \"#B9AA8A\""));
    assert!(!activity.contains("body.SetPadding(padding, padding, padding, padding)"));

    for required in [
        "public enum MobileWidthClass",
        "public readonly record struct MobileLayoutPlan",
        "public readonly record struct MobileResolvedInsets",
        "Compact",
        "Medium",
        "Expanded",
        "MediumBreakpointDp = 600",
        "ExpandedBreakpointDp = 840",
        "MinimumTouchTargetDp = 48",
        "Math.Clamp(fontScale, 1, MaximumLayoutFontScale)",
        "safeArea.Top",
        "safeArea.Bottom",
        "safeArea.Left - contentOuterMargin",
        "effectiveHeight >= MinimumTwoPaneHeightDp",
        "public static void VerifyContract()",
    ] {
        assert!(
            policy.contains(required),
            "mobile layout policy lost {required}"
        );
    }
    assert!(conformance.contains("MobileLayoutPolicy.VerifyContract();"));
    assert!(conformance.contains("mobile_layout_policy=true"));
    assert!(conformance.contains("value_layout_plan=true"));
    assert!(conformance.contains("minimum_touch_dp=48"));
}

#[test]
fn android_native_controls_consume_shared_ui_documents_and_typed_form_events() {
    let activity = source("apps/leserpent-mobile/src/Leserpent.Mobile.Android/MainActivity.cs");
    let binding =
        source("apps/leserpent-mobile/src/Leserpent.MobileCore/MobileUiDocumentBinding.cs");
    let lifecycle =
        source("apps/leserpent-mobile/src/Leserpent.MobileCore/MobileRemoteLifecycle.cs");
    let coordinator =
        source("apps/leserpent-mobile/src/Leserpent.MobileCore/MobileApplicationCoordinator.cs");
    let conformance = source("apps/leserpent-mobile/src/Leserpent.MobileConformance/Program.cs");

    for required in [
        "MobileUiDocumentBinding.Project(",
        "RemoteDocumentProjection.Project(feed).Document",
        "RemoteWorkspaceDocumentProjection.Project(workspace)",
        "RenderNode(document, cards[index], snapshot)",
        "ShowParameterizedFormAsync(source, node, form, intent.Runtime)",
        "source.ResolveSubmission(node.Id, values, currentFeed)",
        "ShowConfirmationAsync(",
        "coordinator.LoadWorkspaceAsync(runtime.Id, lifetime.Token)",
        "coordinator.ExecuteMutationAsync(intent, lifetime.Token)",
        "Values remain local until a validated submit event and explicit confirmation.",
    ] {
        assert!(
            activity.contains(required),
            "Android native document adapter lost {required}"
        );
    }
    assert!(
        !activity.contains("var runtimes = snapshot.Remote?.Feed.Runtimes")
            && !activity.contains("foreach (var runtime in runtimes)")
            && !activity.contains("new RemoteWorkspaceClient")
            && !activity.contains("new RemoteMutationClient"),
        "Android returned to a frontend-owned runtime projection"
    );
    for required in [
        "operationStatus = WorkspaceFailure(error);",
        "operationStatus = MutationFailure(error);",
        "ConnectionFailure(error)",
        "Workspace network request failed safely.",
        "Remote change failed safely.",
        "Network connection failed safely.",
    ] {
        assert!(
            activity.contains(required),
            "Android operator-error boundary lost {required}"
        );
    }
    assert!(
        !activity.contains("Workspace blocked: {Safe(error.Message)}")
            && !activity.contains("operationStatus = Safe(error.Message);")
            && !activity.contains("Connection blocked: {Safe(error.Message)}"),
        "Android rendered an unclassified transport error"
    );

    for required in [
        "public sealed record MobileUiNodeBinding",
        "private readonly SemanticRenderer renderer",
        "renderer.Mount(document)",
        "RemoteUiActionRouter.ResolveActivation(",
        "renderer.CreateFormSubmission(nodeId, values)",
        "RemoteUiActionRouter.ResolveSubmission(",
        "source.Root.Children.Clear()",
        "mobile UI binding did not isolate itself from source mutation",
    ] {
        assert!(
            binding.contains(required),
            "mobile UI binding lost {required}"
        );
    }
    for required in [
        "Task<RemoteWorkspaceSnapshot> LoadWorkspaceAsync(",
        "Task<RemoteMutationResult> ExecuteMutationAsync(",
        "UseForegroundSessionAsync(",
        "MobileRemoteGenerationRetiredException",
        "ReferenceEquals(session, active)",
    ] {
        assert!(
            lifecycle.contains(required),
            "mobile operation lifecycle lost {required}"
        );
    }
    for required in [
        "private const string MobilePrincipal = \"leserpent-mobile\"",
        "private RemoteMutationCoordinator mutationCoordinator = new()",
        "owner.Begin(",
        "owner.Confirm(",
        "owner.Accept(",
        "owner.CompleteFailure(",
    ] {
        assert!(
            coordinator.contains(required),
            "mobile mutation coordinator lost {required}"
        );
    }
    for required in [
        "MobileUiDocumentBinding.VerifyContract();",
        "native_parameterized_form=true",
        "native_form_event_routing=true",
        "native_typed_deployment=true",
        "mobile_operation_generation_fence=true",
    ] {
        assert!(
            conformance.contains(required),
            "mobile conformance lost {required}"
        );
    }
}
