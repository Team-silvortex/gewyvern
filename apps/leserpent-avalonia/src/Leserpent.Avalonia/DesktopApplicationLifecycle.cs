using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Input;

internal static class DesktopApplicationLifecycle
{
    private static readonly string[] RequiredMenuItems =
        ["About Leserpent", "Learning Center...", "Connection...", "Language...", "Show Leserpent", "Quit Leserpent"];

    public static void Configure(
        Application application,
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopLocalization localization,
        Action reopenMainWindow,
        Action manageConnection,
        Action languageApplied)
    {
        if (!OperatingSystem.IsMacOS())
        {
            return;
        }

        desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;
        InstallMenu(
            application,
            desktop,
            localization,
            reopenMainWindow,
            manageConnection,
            languageApplied);
        localization.Changed += (_, _) => InstallMenu(
            application,
            desktop,
            localization,
            reopenMainWindow,
            manageConnection,
            languageApplied);

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

    private static void InstallMenu(
        Application application,
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopLocalization localization,
        Action reopenMainWindow,
        Action manageConnection,
        Action languageApplied)
    {
        var appMenu = new NativeMenu();
        appMenu.Items.Add(Item(
            localization.Text(DesktopTextKey.AboutLeserpent),
            () => ShowAbout(desktop, localization)));
        appMenu.Items.Add(Item(
            localization.Text(DesktopTextKey.LearningCenter),
            () => ShowTutorial(desktop, localization)));
        appMenu.Items.Add(new NativeMenuItemSeparator());
        appMenu.Items.Add(Item(
            localization.Text(DesktopTextKey.Connection),
            manageConnection,
            new KeyGesture(Key.OemComma, KeyModifiers.Meta)));
        appMenu.Items.Add(Item(
            localization.Text(DesktopTextKey.Language),
            () => ShowLanguageSettings(desktop, localization, languageApplied)));
        appMenu.Items.Add(new NativeMenuItemSeparator());
        appMenu.Items.Add(Item(
            localization.Text(DesktopTextKey.QuitLeserpent),
            () => desktop.TryShutdown(0),
            new KeyGesture(Key.Q, KeyModifiers.Meta)));

        var windowMenu = new NativeMenu();
        windowMenu.Items.Add(Item(
            localization.Text(DesktopTextKey.ShowLeserpent),
            () => ShowMainWindow(desktop, reopenMainWindow),
            new KeyGesture(Key.D0, KeyModifiers.Meta)));

        var menu = new NativeMenu();
        menu.Items.Add(new NativeMenuItem("Leserpent") { Menu = appMenu });
        menu.Items.Add(new NativeMenuItem("Window") { Menu = windowMenu });
        NativeDock.SetMenu(application, menu);
    }

    public static void VerifyContract()
    {
        DesktopTutorialWindow.VerifyContentContract();
        DesktopLocalization.VerifyContract();
        if (RequiredMenuItems.Length != 6
            || RequiredMenuItems.Distinct(StringComparer.Ordinal).Count() != 6
            || !RequiredMenuItems.Contains("Learning Center...", StringComparer.Ordinal)
            || !RequiredMenuItems.Contains("Language...", StringComparer.Ordinal)
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
        if (desktop.MainWindow is Window mainWindow
            && mainWindow is not DesktopAboutWindow
            && mainWindow is not DesktopTutorialWindow
            && mainWindow is not DesktopLanguageWindow
            && mainWindow.IsVisible)
        {
            existing = mainWindow;
        }
        else
        {
            existing = desktop.Windows.FirstOrDefault(
                window => window is not DesktopAboutWindow
                    && window is not DesktopTutorialWindow
                    && window is not DesktopLanguageWindow
                    && window.IsVisible);
        }

        if (existing is not null)
        {
            existing.Show();
            existing.Activate();
            return;
        }
        reopenMainWindow();
    }

    internal static void ShowTutorial(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopLocalization localization)
    {
        var existing = desktop.Windows.OfType<DesktopTutorialWindow>().FirstOrDefault();
        if (existing is not null)
        {
            existing.Show();
            existing.Activate();
            return;
        }
        var tutorial = new DesktopTutorialWindow(localization);
        var owner = desktop.MainWindow is { IsVisible: true } mainWindow
            ? mainWindow
            : desktop.Windows.FirstOrDefault(window =>
                window is not DesktopAboutWindow
                && window is not DesktopTutorialWindow
                && window is not DesktopLanguageWindow
                && window.IsVisible);
        if (owner is null)
        {
            tutorial.Show();
        }
        else
        {
            tutorial.Show(owner);
        }
    }

    internal static void ShowLanguageSettings(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopLocalization localization,
        Action applied)
    {
        var existing = desktop.Windows.OfType<DesktopLanguageWindow>().FirstOrDefault();
        if (existing is not null)
        {
            existing.Show();
            existing.Activate();
            return;
        }
        var window = new DesktopLanguageWindow(localization, applied);
        var owner = desktop.MainWindow is { IsVisible: true } mainWindow
            ? mainWindow
            : desktop.Windows.FirstOrDefault(candidate =>
                candidate is not DesktopAboutWindow
                && candidate is not DesktopTutorialWindow
                && candidate is not DesktopLanguageWindow
                && candidate.IsVisible);
        if (owner is null)
        {
            window.Show();
        }
        else
        {
            window.Show(owner);
        }
    }

    private static void ShowAbout(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopLocalization localization)
    {
        var existing = desktop.Windows.OfType<DesktopAboutWindow>().FirstOrDefault();
        if (existing is not null)
        {
            existing.Activate();
            return;
        }
        new DesktopAboutWindow(localization).Show();
    }
}
