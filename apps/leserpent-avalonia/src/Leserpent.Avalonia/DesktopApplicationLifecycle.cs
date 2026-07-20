using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Input;

internal static class DesktopApplicationLifecycle
{
    private static readonly string[] RequiredMenuItems =
        ["About Leserpent", "Connection...", "Show Leserpent", "Quit Leserpent"];

    public static void Configure(
        Application application,
        IClassicDesktopStyleApplicationLifetime desktop,
        Action reopenMainWindow,
        Action manageConnection)
    {
        if (!OperatingSystem.IsMacOS())
        {
            return;
        }

        desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;
        var appMenu = new NativeMenu();
        appMenu.Items.Add(Item("About Leserpent", () => ShowAbout(desktop)));
        appMenu.Items.Add(new NativeMenuItemSeparator());
        appMenu.Items.Add(Item(
            "Connection...",
            manageConnection,
            new KeyGesture(Key.OemComma, KeyModifiers.Meta)));
        appMenu.Items.Add(new NativeMenuItemSeparator());
        appMenu.Items.Add(Item(
            "Quit Leserpent",
            () => desktop.TryShutdown(0),
            new KeyGesture(Key.Q, KeyModifiers.Meta)));

        var windowMenu = new NativeMenu();
        windowMenu.Items.Add(Item(
            "Show Leserpent",
            () => ShowMainWindow(desktop, reopenMainWindow),
            new KeyGesture(Key.D0, KeyModifiers.Meta)));

        var menu = new NativeMenu();
        menu.Items.Add(new NativeMenuItem("Leserpent") { Menu = appMenu });
        menu.Items.Add(new NativeMenuItem("Window") { Menu = windowMenu });
        NativeDock.SetMenu(application, menu);

        if (application.ApplicationLifetime is IActivatableLifetime activatable)
        {
            activatable.Activated += (_, eventArgs) =>
            {
                if (eventArgs.Kind == ActivationKind.Reopen)
                {
                    ShowMainWindow(desktop, reopenMainWindow);
                }
            };
        }
    }

    public static void VerifyContract()
    {
        if (RequiredMenuItems.Length != 4
            || RequiredMenuItems.Distinct(StringComparer.Ordinal).Count() != 4
            || !RequiredMenuItems.Contains("Quit Leserpent", StringComparer.Ordinal))
        {
            throw new InvalidDataException("desktop application menu contract drifted");
        }
    }

    private static NativeMenuItem Item(
        string header,
        Action action,
        KeyGesture? gesture = null)
    {
        var item = new NativeMenuItem(header) { Gesture = gesture };
        item.Click += (_, _) => action();
        return item;
    }

    private static void ShowMainWindow(
        IClassicDesktopStyleApplicationLifetime desktop,
        Action reopenMainWindow)
    {
        Window? existing = null;
        if (desktop.MainWindow is Window mainWindow && mainWindow is not DesktopAboutWindow && mainWindow.IsVisible)
        {
            existing = mainWindow;
        }
        else
        {
            existing = desktop.Windows.FirstOrDefault(
                window => window is not DesktopAboutWindow && window.IsVisible);
        }

        if (existing is not null)
        {
            existing.Show();
            existing.Activate();
            return;
        }
        reopenMainWindow();
    }

    private static void ShowAbout(IClassicDesktopStyleApplicationLifetime desktop)
    {
        var existing = desktop.Windows.OfType<DesktopAboutWindow>().FirstOrDefault();
        if (existing is not null)
        {
            existing.Activate();
            return;
        }
        new DesktopAboutWindow().Show();
    }
}
