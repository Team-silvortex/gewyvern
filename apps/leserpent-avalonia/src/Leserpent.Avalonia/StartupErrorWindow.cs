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

    public static bool IsExpected(Exception error) => error is
        ArgumentException
        or CryptographicException
        or DllNotFoundException
        or EntryPointNotFoundException
        or InvalidDataException
        or InvalidOperationException
        or IOException
        or PlatformNotSupportedException
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
            ? "The desktop configuration could not be validated."
            : sanitized;
    }
}

internal sealed class StartupErrorWindow : Window
{
    private readonly Button closeButton;
    private readonly TextBlock detailText;
    private readonly TextBlock guidanceText;
    private readonly TextBlock headingText;

    public StartupErrorWindow(string description)
    {
        Title = "Leserpent startup problem";
        Width = 560;
        MinWidth = 380;
        MaxWidth = 720;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        closeButton = new Button
        {
            Content = "Close",
            HorizontalAlignment = HorizontalAlignment.Right,
            Padding = new Thickness(20, 9),
        };
        AutomationProperties.SetAutomationId(closeButton, "startup-error-close");
        AutomationProperties.SetName(closeButton, "Close startup error");
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
            Text = description,
            Foreground = LeserpentTheme.Body,
            FontSize = 14,
            LineHeight = 22,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(detailText, "startup-error-detail");
        AutomationProperties.SetName(detailText, $"Startup error: {description}");

        headingText = new TextBlock
        {
            Text = "Remote console could not start",
            Foreground = LeserpentTheme.Primary,
            FontSize = 24,
            FontWeight = FontWeight.Bold,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(headingText, "startup-error-heading");
        AutomationProperties.SetName(headingText, "Remote console could not start");

        guidanceText = new TextBlock
        {
            Text = "Check the HTTPS origin, CA file, and the endpoint-scoped token in Keychain or Secret Service. Tokens are never shown here.",
            Foreground = LeserpentTheme.Muted,
            FontSize = 13,
            LineHeight = 20,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(guidanceText, "startup-error-guidance");
        AutomationProperties.SetName(
            guidanceText,
            "Check the HTTPS origin, CA file, and endpoint-scoped platform credential");

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
}
