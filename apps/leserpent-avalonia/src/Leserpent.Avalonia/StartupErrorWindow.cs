using System.Security;
using System.Security.Cryptography;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;

internal static class StartupFailure
{
    public const int ExitCode = 2;
    public const string DefaultDescription =
        "The desktop configuration could not be validated.";

    public static bool IsExpected(Exception error) => error is
        ArgumentException
        or CryptographicException
        or DllNotFoundException
        or EntryPointNotFoundException
        or InvalidDataException
        or InvalidOperationException
        or IOException
        or PlatformNotSupportedException
        or RemoteQueryException
        or SecurityException
        or UnauthorizedAccessException;

    public static string Describe(Exception error, params string?[] secrets)
    {
        var redacted = error.Message;
        foreach (var secret in secrets)
        {
            if (!string.IsNullOrEmpty(secret))
            {
                redacted = redacted.Replace(secret, "[redacted]", StringComparison.Ordinal);
            }
        }
        var sanitized = new string(redacted
            .Where(character => !char.IsControl(character))
            .Take(512)
            .ToArray());
        return string.IsNullOrWhiteSpace(sanitized)
            ? DefaultDescription
            : sanitized;
    }
}

internal sealed class StartupErrorWindow : Window
{
    private readonly string description;
    private readonly DesktopLocalization localization;
    private readonly Button closeButton;
    private readonly TextBlock detailText;
    private readonly TextBlock guidanceText;
    private readonly TextBlock headingText;

    public StartupErrorWindow(string description, DesktopLocalization localization)
    {
        this.description = description;
        this.localization = localization;
        Width = 600;
        MinWidth = 380;
        MaxWidth = 760;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        closeButton = new Button
        {
            HorizontalAlignment = HorizontalAlignment.Right,
            Padding = new Thickness(20, 9),
        };
        AutomationProperties.SetAutomationId(closeButton, "startup-error-close");
        closeButton.Click += (_, _) => Close();
        Opened += (_, _) => closeButton.Focus();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close();
            }
        };

        detailText = new TextBlock
        {
            Foreground = LeserpentTheme.Body,
            FontSize = 14,
            LineHeight = 22,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(detailText, "startup-error-detail");

        headingText = new TextBlock
        {
            Foreground = LeserpentTheme.Primary,
            FontSize = 24,
            FontWeight = FontWeight.Bold,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(headingText, "startup-error-heading");

        guidanceText = new TextBlock
        {
            Foreground = LeserpentTheme.Muted,
            FontSize = 13,
            LineHeight = 20,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(guidanceText, "startup-error-guidance");

        Content = new Border
        {
            Padding = new Thickness(32),
            Child = new StackPanel
            {
                Spacing = 16,
                Children =
                {
                    headingText,
                    detailText,
                    new Border
                    {
                        Background = LeserpentTheme.Panel,
                        BorderBrush = LeserpentTheme.PanelBorder,
                        BorderThickness = new Thickness(1),
                        CornerRadius = new CornerRadius(8),
                        Padding = new Thickness(16, 14),
                        Child = guidanceText,
                    },
                    closeButton,
                },
            },
        };
        Closed += (_, _) => localization.Changed -= OnLocalizationChanged;
        localization.Changed += OnLocalizationChanged;
        ApplyLocalization();
    }

    public void VerifyAccessibility()
    {
        var controls = new Control[] { headingText, detailText, guidanceText, closeButton };
        if (controls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))))
        {
            throw new InvalidDataException(
                "startup error control is missing automation metadata");
        }
        if (controls.Select(AutomationProperties.GetAutomationId)
            .Distinct(StringComparer.Ordinal).Count() != controls.Length)
        {
            throw new InvalidDataException(
                "startup error controls have duplicate automation IDs");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("startup recovery window has no control root");
        }
        root.Measure(new Size(Width, 900));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 900)
        {
            throw new InvalidDataException(
                "startup recovery controls exceeded their layout envelope");
        }
    }

    public void ProbeLocalizedPresentation(
        string expectedTitle,
        string expectedHeading,
        string expectedClose,
        string expectedGuidance)
    {
        if (Title != expectedTitle
            || headingText.Text != expectedHeading
            || closeButton.Content as string != expectedClose
            || guidanceText.Text != expectedGuidance
            || AutomationProperties.GetName(closeButton) != Text("a11y.close")
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "startup recovery localized presentation drifted");
        }
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs) =>
        ApplyLocalization();

    private void ApplyLocalization()
    {
        var displayedDescription = description == StartupFailure.DefaultDescription
            ? Text("detail.fallback")
            : description;
        Title = Text("title");
        FlowDirection = localization.FlowDirection;
        closeButton.Content = Text("close");
        headingText.Text = Text("heading");
        detailText.Text = displayedDescription;
        guidanceText.Text = Text("guidance");
        AutomationProperties.SetName(closeButton, Text("a11y.close"));
        AutomationProperties.SetName(
            detailText,
            Format("a11y.detail", displayedDescription));
        AutomationProperties.SetName(headingText, Text("a11y.heading"));
        AutomationProperties.SetName(guidanceText, Text("a11y.guidance"));
    }

    private string Text(string key) =>
        DesktopStartupRecoveryCatalogs.Resolve(localization, key);

    private string Format(string key, params object[] values) =>
        DesktopStartupRecoveryCatalogs.Format(localization, key, values);
}
