using System.IO;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;

internal sealed class HubWindow : Window
{
    private readonly Func<string?> openRemote;
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
        IsVisible = false,
    };
    private readonly Button openRemoteButton = new()
    {
        Content = "Open Remote Console",
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(18, 10),
    };
    private readonly Button openConnectionSettingsButton = new()
    {
        Content = "Connection...",
        Padding = new Thickness(18, 10),
    };

    public HubWindow(
        DesktopConnectionProfile? profile,
        string? initialError,
        Func<string?> openRemote,
        Action openConnectionSettings)
    {
        this.openRemote = openRemote;
        Title = "Leserpent / Hub";
        Width = 640;
        Height = 420;
        MinWidth = 560;
        MinHeight = 340;
        SizeToContent = SizeToContent.Manual;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        CanResize = true;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        var profileSummary = new TextBlock
        {
            Text = ResolveProfileSummary(profile),
            Foreground = LeserpentTheme.Body,
            FontSize = 13,
            TextWrapping = TextWrapping.Wrap,
        };

        statusText.Foreground = string.IsNullOrWhiteSpace(initialError)
            ? LeserpentTheme.Muted
            : LeserpentTheme.Destructive;
        statusText.Text = string.IsNullOrWhiteSpace(initialError)
            ? "Ready for manual deployment or runtime inspection."
            : Safe(initialError);
        statusText.IsVisible = true;

        AutomationProperties.SetAutomationId(profileSummary, "hub-connection-summary");
        AutomationProperties.SetName(profileSummary, "Saved connection summary");
        AutomationProperties.SetAutomationId(openRemoteButton, "hub-open-remote");
        AutomationProperties.SetName(openRemoteButton, "Open remote console");
        AutomationProperties.SetAutomationId(openConnectionSettingsButton, "hub-open-connection-settings");
        AutomationProperties.SetName(openConnectionSettingsButton, "Open connection settings");
        AutomationProperties.SetAutomationId(statusText, "hub-status");
        AutomationProperties.SetName(statusText, "Hub status");

        openRemoteButton.Click += (_, _) => OpenRemote();
        openConnectionSettingsButton.Click += (_, _) => openConnectionSettings();

        var actions = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            Spacing = 12,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left,
            Children = { openRemoteButton, openConnectionSettingsButton },
        };

        Content = new Border
        {
            Padding = new Thickness(34, 30),
            Child = new StackPanel
            {
                Spacing = 16,
                Children =
                {
                    new TextBlock
                    {
                        Text = "LESERPENT",
                        Foreground = LeserpentTheme.Accent,
                        FontSize = 14,
                        FontWeight = FontWeight.Bold,
                        LetterSpacing = 2,
                    },
                    new TextBlock
                    {
                        Text = "Control Hub",
                        Foreground = LeserpentTheme.Primary,
                        FontSize = 33,
                        FontWeight = FontWeight.Bold,
                    },
                    new TextBlock
                    {
                        Text = "Select an entry point: open the remote orchestration console if credentials are ready, or configure a saved connection profile first.",
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 13,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new Border
                    {
                        Background = LeserpentTheme.Panel,
                        BorderBrush = LeserpentTheme.PanelBorder,
                        BorderThickness = new Thickness(1),
                        CornerRadius = new CornerRadius(8),
                        Padding = new Thickness(16, 14),
                        Child = new StackPanel
                        {
                            Spacing = 10,
                            Children =
                            {
                                new TextBlock
                                {
                                    Text = "Saved Connection",
                                    Foreground = LeserpentTheme.Primary,
                                    FontSize = 12,
                                    FontWeight = FontWeight.SemiBold,
                                },
                                profileSummary,
                            },
                        },
                    },
                    actions,
                    statusText,
                },
            },
        };

        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close();
            }
        };
    }

    private void OpenRemote()
    {
        openRemoteButton.IsEnabled = false;
        statusText.Text = "Launching remote console...";
        statusText.Foreground = LeserpentTheme.Primary;
        statusText.IsVisible = true;
        var error = openRemote();
        if (error is null)
        {
            return;
        }
        statusText.Text = Safe(error);
        statusText.Foreground = LeserpentTheme.Destructive;
        openRemoteButton.IsEnabled = true;
    }

    private static string ResolveProfileSummary(DesktopConnectionProfile? profile)
    {
        return profile is null
            ? "No saved connection profile yet. Configure one in Connection..."
            : $"Endpoint: {Safe(profile.Endpoint)}\nCA: {Path.GetFileName(profile.CertificateAuthorityPath)}";
    }

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(512)
        .ToArray());
}
