using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Media;

internal sealed class DesktopAboutWindow : Window
{
    public DesktopAboutWindow()
    {
        Title = "About Leserpent";
        Width = 420;
        Height = 270;
        CanResize = false;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        var close = new Button
        {
            Content = "Close",
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center,
            Padding = new Thickness(24, 8),
        };
        close.Click += (_, _) => Close();
        AutomationProperties.SetAutomationId(close, "desktop-about-close");
        AutomationProperties.SetName(close, "Close About Leserpent");

        Content = new Border
        {
            Padding = new Thickness(34, 28),
            Child = new StackPanel
            {
                Spacing = 10,
                HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center,
                Children =
                {
                    new TextBlock
                    {
                        Text = "LESERPENT",
                        Foreground = LeserpentTheme.Accent,
                        FontSize = 26,
                        FontWeight = FontWeight.Bold,
                        LetterSpacing = 2,
                    },
                    new TextBlock
                    {
                        Text = $"Version {typeof(DesktopAboutWindow).Assembly.GetName().Version?.ToString(3) ?? "unknown"}",
                        Foreground = LeserpentTheme.Primary,
                        FontSize = 14,
                    },
                    new TextBlock
                    {
                        Text = "Native orchestration and model-driven control for Gewyvern.",
                        Foreground = LeserpentTheme.Muted,
                        TextAlignment = TextAlignment.Center,
                        TextWrapping = TextWrapping.Wrap,
                        MaxWidth = 330,
                    },
                    close,
                },
            },
        };
    }
}
