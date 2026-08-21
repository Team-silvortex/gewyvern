using Foundation;
using UIKit;

[Register("SceneDelegate")]
public sealed class SceneDelegate : UIResponder, IUIWindowSceneDelegate
{
    private MobileHubViewController? hub;
    private UIView? privacyShield;

    [Export("window")]
    public UIWindow? Window { get; set; }

    [Export("scene:willConnectToSession:options:")]
    public void WillConnect(
        UIScene scene,
        UISceneSession session,
        UISceneConnectionOptions connectionOptions)
    {
        _ = session;
        _ = connectionOptions;
        if (scene is not UIWindowScene windowScene)
        {
            return;
        }
        HidePrivacyShield();
        if (Window is not null)
        {
            Window.RootViewController = null;
        }
        Window = new UIWindow(windowScene);
        hub ??= new MobileHubViewController(
            new IosConnectionProfileStore(),
            new MobileCredentialVault(new IosKeychainSecretStore()));
        Window.RootViewController = hub;
        Window.MakeKeyAndVisible();
        ShowPrivacyShield();
    }

    [Export("sceneDidBecomeActive:")]
    public async void DidBecomeActive(UIScene scene)
    {
        _ = scene;
        HidePrivacyShield();
        if (hub is not null)
        {
            await hub.EnterForegroundAsync();
        }
    }

    [Export("sceneWillResignActive:")]
    public void WillResignActive(UIScene scene)
    {
        _ = scene;
        ShowPrivacyShield();
    }

    [Export("sceneDidEnterBackground:")]
    public async void DidEnterBackground(UIScene scene)
    {
        _ = scene;
        if (hub is not null)
        {
            await hub.EnterBackgroundAsync();
        }
    }

    [Export("sceneDidDisconnect:")]
    public async void DidDisconnect(UIScene scene)
    {
        _ = scene;
        if (hub is not null)
        {
            await hub.EnterBackgroundAsync();
        }
    }

    private void ShowPrivacyShield()
    {
        if (Window is null || privacyShield is not null)
        {
            return;
        }
        var shield = new UIView(Window.Bounds)
        {
            BackgroundColor = UIColor.FromRGB(17, 16, 13),
            AutoresizingMask = UIViewAutoresizing.FlexibleWidth
                | UIViewAutoresizing.FlexibleHeight,
        };
        var title = new UILabel(shield.Bounds)
        {
            Text = "LESERPENT",
            TextAlignment = UITextAlignment.Center,
            TextColor = UIColor.FromRGB(255, 178, 41),
            Font = UIFont.BoldSystemFontOfSize(24)!,
            AutoresizingMask = UIViewAutoresizing.FlexibleWidth
                | UIViewAutoresizing.FlexibleHeight,
        };
        shield.AddSubview(title);
        Window.AddSubview(shield);
        privacyShield = shield;
    }

    private void HidePrivacyShield()
    {
        privacyShield?.RemoveFromSuperview();
        privacyShield?.Dispose();
        privacyShield = null;
    }
}
