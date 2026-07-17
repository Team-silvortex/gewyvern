using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;

internal sealed class RemoteRuntimeWorkspaceWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly RemoteWorkspaceClient client;
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button reloadButton = new()
    {
        Content = "Reload",
        Padding = new Thickness(12, 6),
    };
    private bool loadInFlight;
    private ulong loadedRevision;
    private ulong desiredRevision;

    public RemoteRuntimeWorkspaceWindow(
        RemoteClientOptions options,
        RemoteRuntimeProjection runtime,
        string principal,
        Action<string> actionInvoked)
    {
        RuntimeId = runtime.Id;
        this.principal = principal;
        client = new RemoteWorkspaceClient(options);
        renderer = new AvaloniaDocumentRenderer(actionInvoked);
        Width = 760;
        Height = 620;
        MinWidth = 520;
        MinHeight = 420;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        Title = $"{Safe(runtime.Name)} / Leserpent";
        AutomationProperties.SetAutomationId(statusText, "runtime-workspace-status");
        AutomationProperties.SetName(statusText, "Runtime workspace query status");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(reloadButton, "runtime-workspace-reload");
        AutomationProperties.SetName(reloadButton, "Reload runtime workspace");
        AutomationProperties.SetHelpText(
            reloadButton,
            "Reloads status, history, and logs through one revision-consistent query group. Shortcut: F5.");
        reloadButton.Click += (_, _) => _ = ReloadAsync();
        var status = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(20, 10),
            Child = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
                ColumnSpacing = 12,
                Children = { statusText, reloadButton },
            },
        };
        Grid.SetColumn(reloadButton, 1);
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,*"),
            Children = { status, renderer.Surface },
        };
        Grid.SetRow(renderer.Surface, 1);
        Opened += (_, _) => _ = ReloadAsync();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.F5 && reloadButton.IsEnabled)
            {
                eventArgs.Handled = true;
                _ = ReloadAsync();
            }
        };
        Closed += (_, _) =>
        {
            lifetime.Cancel();
            client.Dispose();
            lifetime.Dispose();
        };
    }

    public string RuntimeId { get; }

    public void SetRefreshAvailability(bool enabled, string? reason) =>
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, enabled, reason);

    public void ReloadIfOlder(ulong revision)
    {
        desiredRevision = Math.Max(desiredRevision, revision);
        if (revision > loadedRevision && !loadInFlight)
        {
            _ = ReloadAsync();
        }
    }

    private async Task ReloadAsync()
    {
        if (loadInFlight || lifetime.IsCancellationRequested)
        {
            return;
        }
        loadInFlight = true;
        var loaded = false;
        reloadButton.IsEnabled = false;
        SetStatus("Loading authenticated runtime snapshot...", LeserpentTheme.Primary);
        try
        {
            var snapshot = await client.LoadAsync(RuntimeId, principal, lifetime.Token);
            renderer.Mount(RemoteWorkspaceDocumentProjection.Project(snapshot));
            loadedRevision = snapshot.Revision;
            loaded = true;
            SetStatus(
                $"Live workspace at revision {snapshot.Revision}",
                LeserpentTheme.Accent);
        }
        catch (RemoteQueryException error)
        {
            ShowFailure($"Query rejected ({Safe(error.Code)}): {Safe(error.Message)}");
        }
        catch (InvalidDataException error)
        {
            ShowFailure($"Query response rejected: {Safe(error.Message)}");
        }
        catch (ArgumentException error)
        {
            ShowFailure($"Query blocked: {Safe(error.Message)}");
        }
        catch (OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            ShowFailure("Query timed out; no partial workspace was retained");
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            // Window shutdown owns this cancellation.
        }
        catch (HttpRequestException)
        {
            ShowFailure("Query failed over the authenticated HTTPS connection");
        }
        catch (ObjectDisposedException) when (lifetime.IsCancellationRequested)
        {
            // The HTTP client may be disposed while the window is closing.
        }
        finally
        {
            loadInFlight = false;
            reloadButton.IsEnabled = !lifetime.IsCancellationRequested;
            if (loaded && desiredRevision > loadedRevision)
            {
                _ = ReloadAsync();
            }
        }
    }

    private void ShowFailure(string message)
    {
        SetStatus(message, LeserpentTheme.Destructive);
    }

    private void SetStatus(string text, IBrush foreground)
    {
        statusText.Text = text;
        statusText.Foreground = foreground;
        AutomationProperties.SetName(statusText, $"Runtime workspace query status: {text}");
        AutomationProperties.SetLiveSetting(
            statusText,
            ReferenceEquals(foreground, LeserpentTheme.Destructive)
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
    }

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
