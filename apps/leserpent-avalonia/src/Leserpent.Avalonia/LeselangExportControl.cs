using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input.Platform;
using Avalonia.Media;

internal sealed class LeselangExportControl : StackPanel
{
    private readonly Button copy;
    private readonly TextBox preview;
    private readonly TextBlock status;

    public LeselangExportControl(string automationPrefix, string? source = null)
    {
        Spacing = 8;
        preview = new TextBox
        {
            AcceptsReturn = true,
            IsReadOnly = true,
            FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
            FontSize = 12,
            MaxHeight = 150,
            TextWrapping = TextWrapping.Wrap,
        };
        copy = new Button
        {
            Content = "Copy Leselang",
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left,
            Padding = new Thickness(14, 7),
        };
        status = new TextBlock
        {
            Foreground = LeserpentTheme.Muted,
            FontSize = 12,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(
            preview,
            $"{automationPrefix}-leselang-preview");
        AutomationProperties.SetName(preview, "Equivalent Leselang source");
        AutomationProperties.SetAutomationId(
            copy,
            $"{automationPrefix}-leselang-copy");
        AutomationProperties.SetName(copy, "Copy equivalent Leselang source");
        AutomationProperties.SetHelpText(
            copy,
            "Copies the code-equivalent operation without executing it.");
        AutomationProperties.SetAutomationId(
            status,
            $"{automationPrefix}-leselang-copy-status");
        AutomationProperties.SetName(status, "Leselang copy status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);
        copy.Click += async (_, _) => await CopyAsync();
        Children.Add(new TextBlock
        {
            Text = "Equivalent Leselang",
            Foreground = LeserpentTheme.Body,
            FontWeight = FontWeight.SemiBold,
        });
        Children.Add(preview);
        Children.Add(copy);
        Children.Add(status);
        Update(source);
    }

    public void Update(string? source)
    {
        preview.Text = source ?? string.Empty;
        copy.IsEnabled = !string.IsNullOrEmpty(source);
        status.Text = string.Empty;
        AutomationProperties.SetName(status, "Leselang copy status");
    }

    private async Task CopyAsync()
    {
        var source = preview.Text;
        var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
        if (string.IsNullOrEmpty(source) || clipboard is null)
        {
            SetStatus("Clipboard unavailable.", true);
            return;
        }
        try
        {
            await clipboard.SetTextAsync(source);
            SetStatus("Leselang copied. No operation was executed.", false);
        }
        catch (Exception)
        {
            SetStatus("Leselang copy failed safely.", true);
        }
    }

    private void SetStatus(string value, bool failed)
    {
        status.Text = value;
        status.Foreground = failed
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Accent;
        AutomationProperties.SetName(status, value);
        AutomationProperties.SetLiveSetting(
            status,
            failed ? AutomationLiveSetting.Assertive : AutomationLiveSetting.Polite);
    }
}
