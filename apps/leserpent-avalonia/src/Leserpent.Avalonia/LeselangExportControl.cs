using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input.Platform;
using Avalonia.Media;

internal sealed class LeselangExportControl : StackPanel
{
    private static readonly TimeSpan ExportDebounce = TimeSpan.FromMilliseconds(160);
    private readonly Button copy;
    private readonly TextBox preview;
    private readonly TextBlock status;
    private readonly TextBlock label;
    private readonly DesktopLocalization localization;
    private CancellationTokenSource? exportRequest;
    private string currentStatusKey = "leselang.status.invalid";
    private bool currentStatusFailed;

    public LeselangExportControl(
        string automationPrefix,
        Func<CancellationToken, Task<string>>? sourceLoader = null,
        DesktopLocalization? localization = null)
    {
        this.localization = localization ?? DesktopLocalization.ForVerification();
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
        AutomationProperties.SetAutomationId(
            copy,
            $"{automationPrefix}-leselang-copy");
        AutomationProperties.SetAutomationId(
            status,
            $"{automationPrefix}-leselang-copy-status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);
        copy.Click += async (_, _) => await CopyAsync();
        label = new TextBlock
        {
            Foreground = LeserpentTheme.Body,
            FontWeight = FontWeight.SemiBold,
        };
        Children.Add(label);
        Children.Add(preview);
        Children.Add(copy);
        Children.Add(status);
        this.localization.Changed += OnLocalizationChanged;
        DetachedFromVisualTree += OnDetachedFromVisualTree;
        ApplyLocalization();
        Update(sourceLoader);
    }

    public void Update(Func<CancellationToken, Task<string>>? sourceLoader)
    {
        exportRequest?.Cancel();
        exportRequest?.Dispose();
        exportRequest = null;
        preview.Text = string.Empty;
        copy.IsEnabled = false;
        if (sourceLoader is null)
        {
            SetStatus("leselang.status.invalid", false);
            return;
        }
        var request = new CancellationTokenSource();
        exportRequest = request;
        SetStatus("leselang.status.generating", false);
        _ = LoadAsync(sourceLoader, request);
    }

    private async Task LoadAsync(
        Func<CancellationToken, Task<string>> sourceLoader,
        CancellationTokenSource request)
    {
        try
        {
            await Task.Delay(ExportDebounce, request.Token);
            var source = await sourceLoader(request.Token);
            if (!ReferenceEquals(exportRequest, request) || request.IsCancellationRequested)
            {
                return;
            }
            preview.Text = source;
            copy.IsEnabled = !string.IsNullOrEmpty(source);
            SetStatus("leselang.status.generated", false);
        }
        catch (OperationCanceledException) when (request.IsCancellationRequested)
        {
        }
        catch (Exception) when (!request.IsCancellationRequested)
        {
            if (ReferenceEquals(exportRequest, request))
            {
                preview.Text = string.Empty;
                copy.IsEnabled = false;
                SetStatus(
                    "leselang.status.unavailable",
                    true);
            }
        }
        finally
        {
            if (ReferenceEquals(exportRequest, request))
            {
                exportRequest = null;
                request.Dispose();
            }
        }
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        ApplyLocalization();
    }

    private void OnDetachedFromVisualTree(
        object? sender,
        Avalonia.VisualTreeAttachmentEventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        localization.Changed -= OnLocalizationChanged;
        CancelExport();
    }

    private void ApplyLocalization()
    {
        label.Text = DesktopRemoteOperationCatalogs.Resolve(
            localization,
            "leselang.label");
        copy.Content = DesktopRemoteOperationCatalogs.Resolve(
            localization,
            "leselang.copy");
        AutomationProperties.SetName(
            preview,
            DesktopRemoteOperationCatalogs.Resolve(
                localization,
                "leselang.a11y.preview"));
        AutomationProperties.SetName(
            copy,
            DesktopRemoteOperationCatalogs.Resolve(
                localization,
                "leselang.a11y.copy"));
        AutomationProperties.SetHelpText(
            copy,
            DesktopRemoteOperationCatalogs.Resolve(
                localization,
                "leselang.help.copy"));
        AutomationProperties.SetName(
            status,
            DesktopRemoteOperationCatalogs.Resolve(
                localization,
                "leselang.a11y.status"));
        ApplyStatus();
    }

    private void CancelExport()
    {
        exportRequest?.Cancel();
        exportRequest?.Dispose();
        exportRequest = null;
    }

    private async Task CopyAsync()
    {
        var source = preview.Text;
        var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
        if (string.IsNullOrEmpty(source) || clipboard is null)
        {
            SetStatus("leselang.status.clipboard_unavailable", true);
            return;
        }
        try
        {
            await clipboard.SetTextAsync(source);
            SetStatus("leselang.status.copied", false);
        }
        catch (Exception)
        {
            SetStatus("leselang.status.copy_failed", true);
        }
    }

    private void SetStatus(string key, bool failed)
    {
        currentStatusKey = key;
        currentStatusFailed = failed;
        ApplyStatus();
    }

    private void ApplyStatus()
    {
        var value = DesktopRemoteOperationCatalogs.Resolve(
            localization,
            currentStatusKey);
        status.Text = value;
        status.Foreground = currentStatusFailed
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Accent;
        AutomationProperties.SetLiveSetting(
            status,
            currentStatusFailed
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
    }
}
