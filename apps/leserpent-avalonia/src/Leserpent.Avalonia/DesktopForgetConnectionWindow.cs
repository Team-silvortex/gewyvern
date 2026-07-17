using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;

internal sealed class DesktopForgetConnectionWindow : Window
{
    private readonly Func<string?> forget;
    private readonly List<Control> auditedControls = [];
    private readonly TextBlock status = new()
    {
        Foreground = LeserpentTheme.Destructive,
        FontSize = 13,
        IsVisible = false,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button forgetButton = new()
    {
        Content = "Forget connection",
        Background = LeserpentTheme.Destructive,
        Foreground = Brushes.White,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(16, 8),
    };

    public DesktopForgetConnectionWindow(string endpoint, Func<string?> forget)
    {
        this.forget = forget;
        Title = "Leserpent / Forget Connection";
        Width = 520;
        MinWidth = 440;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        var cancel = new Button
        {
            Content = "Cancel",
            Padding = new Thickness(16, 8),
        };
        cancel.Click += (_, _) => Close(false);
        forgetButton.Click += (_, _) => ConfirmForget();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close(false);
            }
        };

        AutomationProperties.SetAutomationId(cancel, "desktop-forget-cancel");
        AutomationProperties.SetName(cancel, "Cancel forgetting saved connection");
        AutomationProperties.SetAutomationId(forgetButton, "desktop-forget-confirm");
        AutomationProperties.SetName(forgetButton, "Confirm forgetting saved connection");
        AutomationProperties.SetAutomationId(status, "desktop-forget-status");
        AutomationProperties.SetName(status, "Forget connection status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);
        auditedControls.AddRange([cancel, forgetButton, status]);

        var buttons = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Spacing = 12,
            Children = { cancel, forgetButton },
        };
        Content = new Border
        {
            Padding = new Thickness(30, 26),
            Child = new StackPanel
            {
                Spacing = 14,
                Children =
                {
                    new TextBlock
                    {
                        Text = "Forget this connection?",
                        Foreground = LeserpentTheme.Primary,
                        FontSize = 22,
                        FontWeight = FontWeight.Bold,
                    },
                    new TextBlock
                    {
                        Text = endpoint,
                        Foreground = LeserpentTheme.Accent,
                        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
                        FontSize = 12,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new TextBlock
                    {
                        Text = "This removes the saved non-secret profile and this endpoint's Keychain or Secret Service credential. Environment variables and credentials for other endpoints are not changed.",
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 13,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    status,
                    buttons,
                },
            },
        };
    }

    public void VerifyAccessibility()
    {
        var ids = new HashSet<string>(StringComparer.Ordinal);
        if (auditedControls.Count != 3
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))
                || !ids.Add(AutomationProperties.GetAutomationId(control)!))
            || AutomationProperties.GetLiveSetting(status) != AutomationLiveSetting.Assertive)
        {
            throw new InvalidDataException(
                "forget connection accessibility contract drifted");
        }
    }

    private void ConfirmForget()
    {
        forgetButton.IsEnabled = false;
        var error = forget();
        if (error is null)
        {
            Close(true);
            return;
        }
        status.Text = new string(error
            .Where(character => !char.IsControl(character))
            .Take(512)
            .ToArray());
        status.IsVisible = true;
        AutomationProperties.SetName(status, $"Forget connection failed: {status.Text}");
        forgetButton.IsEnabled = true;
    }
}
