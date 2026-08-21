using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;

internal sealed class DesktopForgetConnectionWindow : Window
{
    private readonly DesktopLocalization localization;
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
        Background = LeserpentTheme.Destructive,
        Foreground = Brushes.White,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(16, 8),
    };
    private readonly Button cancelButton = new()
    {
        Padding = new Thickness(16, 8),
    };
    private readonly TextBlock headingText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 22,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock descriptionText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 13,
        TextWrapping = TextWrapping.Wrap,
    };

    public DesktopForgetConnectionWindow(
        string endpoint,
        Func<string?> forget,
        DesktopLocalization localization)
    {
        this.localization = localization;
        this.forget = forget;
        Width = 520;
        MinWidth = 440;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        cancelButton.Click += (_, _) => Close(false);
        forgetButton.Click += (_, _) => ConfirmForget();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close(false);
            }
        };
        Closed += (_, _) => localization.Changed -= OnLocalizationChanged;

        AutomationProperties.SetAutomationId(cancelButton, "desktop-forget-cancel");
        AutomationProperties.SetAutomationId(forgetButton, "desktop-forget-confirm");
        AutomationProperties.SetAutomationId(status, "desktop-forget-status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);
        auditedControls.AddRange([cancelButton, forgetButton, status]);

        var buttons = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Spacing = 12,
            Children = { cancelButton, forgetButton },
        };
        Content = new Border
        {
            Padding = new Thickness(30, 26),
            Child = new StackPanel
            {
                Spacing = 14,
                Children =
                {
                    headingText,
                    new TextBlock
                    {
                        Text = endpoint,
                        Foreground = LeserpentTheme.Accent,
                        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
                        FontSize = 12,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    descriptionText,
                    status,
                    buttons,
                },
            },
        };
        localization.Changed += OnLocalizationChanged;
        ApplyLocalization();
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

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("forget connection window has no control root");
        }
        root.Measure(new Size(Width, 700));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 700)
        {
            throw new InvalidDataException(
                "forget connection controls exceeded their layout envelope");
        }
    }

    public void ProbeLocalizedPresentation(
        string expectedTitle,
        string expectedHeading,
        string expectedAction)
    {
        if (Title != expectedTitle
            || headingText.Text != expectedHeading
            || forgetButton.Content as string != expectedAction
            || AutomationProperties.GetName(forgetButton) != expectedAction
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "forget connection localized presentation drifted");
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
        AutomationProperties.SetName(
            status,
            DesktopConnectionCatalogs.Format(
                localization,
                "forget.failed",
                status.Text));
        forgetButton.IsEnabled = true;
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs) =>
        ApplyLocalization();

    private void ApplyLocalization()
    {
        Title = Text("forget.title");
        FlowDirection = localization.FlowDirection;
        cancelButton.Content = Text("cancel");
        forgetButton.Content = Text("forget.action");
        headingText.Text = Text("forget.heading");
        descriptionText.Text = Text("forget.body");
        AutomationProperties.SetName(cancelButton, Text("cancel"));
        AutomationProperties.SetName(forgetButton, Text("forget.action"));
        AutomationProperties.SetName(
            status,
            status.IsVisible && !string.IsNullOrWhiteSpace(status.Text)
                ? DesktopConnectionCatalogs.Format(
                    localization,
                    "forget.failed",
                    status.Text)
                : Text("forget.status.name"));
    }

    private string Text(string key) =>
        DesktopConnectionCatalogs.Resolve(localization, key);
}
