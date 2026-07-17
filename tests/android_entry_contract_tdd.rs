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

    for required in [
        "<OutputType>Exe</OutputType>",
        "<TargetFramework>net10.0-android</TargetFramework>",
        "<ApplicationId>org.gewyvern.leserpent</ApplicationId>",
    ] {
        assert!(
            project.contains(required),
            "Android project lost {required}"
        );
    }
    for required in [
        "MainLauncher = true",
        "UsesPermission(Android.Manifest.Permission.Internet)",
        "new AndroidKeystoreSecretStore(this)",
        "new MobileApplicationCoordinator(",
        "WindowManagerFlags.Secure",
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
        "RemoteClientOptions.ParseEndpoint",
        "X509Certificate2.CreateFromPem",
        "PRIVATE KEY",
        "applicationContext.FilesDir",
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
}
