using Foundation;
using UIKit;

[Register("AppDelegate")]
public sealed class AppDelegate : UIApplicationDelegate
{
    public override bool FinishedLaunching(
        UIApplication application,
        NSDictionary? launchOptions)
    {
        _ = application;
        _ = launchOptions;
#if DEBUG
        if (Environment.GetCommandLineArgs().Contains(
                "--leserpent-keychain-proof",
                StringComparer.Ordinal))
        {
            _ = IosPlatformProof.RunKeychainAsync();
        }
#endif
        return true;
    }

    public override UISceneConfiguration GetConfiguration(
        UIApplication application,
        UISceneSession connectingSceneSession,
        UISceneConnectionOptions options) => new(
            "Default Configuration",
            connectingSceneSession.Role);
}
