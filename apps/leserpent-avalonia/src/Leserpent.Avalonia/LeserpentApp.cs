using System.Text.Json;
using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Styling;
using Avalonia.Themes.Fluent;
using Avalonia.Threading;

internal sealed class LeserpentApp : Application
{
    private const int MaxPayloadBytes = 2 * 1024 * 1024;

    public override void Initialize()
    {
        RequestedThemeVariant = ThemeVariant.Dark;
        Styles.Add(new FluentTheme());
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            if (desktop.Args is ["--verify-startup-error"])
            {
                ConfigureStartupErrorVerification(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is ["--remote", ..])
            {
                ConfigureRemoteWindow(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            var verifyControls = desktop.Args is ["--verify-controls", _];
            var verifyFocusRetention = desktop.Args is ["--verify-focus-retention", _];
            var fixture = LoadFixture(desktop.Args, verifyControls, verifyFocusRetention);
            var window = new MainWindow(fixture);
            desktop.MainWindow = window;
            if (verifyControls)
            {
                window.Opened += (_, _) =>
                {
                    window.ProbeActionAvailability();
                    var accessibility = window.Accessibility;
                    Console.WriteLine(
                        $"Avalonia controls valid: nodes={window.RenderedNodeCount}, "
                        + $"operations={window.AppliedPatchOperations}, "
                        + $"reused={window.ReusedNodeCount}, "
                        + $"virtualized={window.VirtualizedHostCount}, "
                        + $"active_virtualized={window.ActiveVirtualizedHostCount}, "
                        + $"initial_unrealized={window.InitialUnrealizedVirtualItemCount}, "
                        + $"remaining_unrealized={window.UnrealizedVirtualItemCount}, "
                        + $"initial_unrealized_nodes={window.InitialUnrealizedNodeCount}, "
                        + $"remaining_unrealized_nodes={window.UnrealizedNodeCount}, "
                        + $"initial_debugger_cancel_buttons={window.InitialDebuggerCancelButtonCount}, "
                        + $"remaining_debugger_cancel_buttons={window.DebuggerCancelButtonCount}, "
                        + $"disabled_action_probe={window.DisabledActionProbeCount}, "
                        + $"initial_accessibility_actions={window.InitialAccessibility.ActionControls}, "
                        + $"accessibility_controls={accessibility.RealizedControls}, "
                        + $"accessibility_names={accessibility.AutomationNames}, "
                        + $"accessibility_labels={accessibility.ExplicitLabels}, "
                        + $"accessibility_actions={accessibility.ActionControls}, "
                        + $"accessibility_help_texts={accessibility.HelpTexts}, "
                        + $"minimum_contrast={accessibility.MinimumContrastRatio:F3}, "
                        + "accessibility_valid=true, "
                        + $"revision={window.Revision}");
                    DispatcherTimer.RunOnce(
                        () => desktop.Shutdown(0),
                        TimeSpan.FromMilliseconds(100));
                };
            }
            else if (verifyFocusRetention)
            {
                window.Opened += (_, _) =>
                {
                    var nodeId = window.BeginFocusRetentionProbe();
                    DispatcherTimer.RunOnce(
                        () =>
                        {
                            window.CompleteFocusRetentionProbe(nodeId);
                            window.BeginPatchedFocusRetentionProbe(nodeId);
                            DispatcherTimer.RunOnce(
                                () =>
                                {
                                    window.CompleteFocusRetentionProbe(nodeId);
                                    window.ProbeRemovedFocusTarget(nodeId);
                                    Console.WriteLine(
                                        $"Avalonia focus retention valid: node={nodeId}, remount=true, patch_update=true, restored=true, removed_target_safe=true");
                                    desktop.Shutdown(0);
                                },
                                TimeSpan.FromMilliseconds(200));
                        },
                        TimeSpan.FromMilliseconds(200));
                };
            }
        }
        base.OnFrameworkInitializationCompleted();
    }

    private static void ConfigureStartupErrorVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var fixtureToken = new string('s', 32);
        var description = StartupFailure.Describe(
            new InvalidDataException($"Verification failure containing {fixtureToken}"),
            fixtureToken);
        if (description.Contains(fixtureToken, StringComparison.Ordinal)
            || !description.Contains("[redacted]", StringComparison.Ordinal))
        {
            throw new InvalidDataException("startup error token redaction failed");
        }
        var window = new StartupErrorWindow(description);
        window.Opened += (_, _) =>
        {
            window.VerifyAccessibility();
            Console.WriteLine(
                "startup error controls valid: controls=4, automation_ids=4, automation_names=4, token_redacted=true");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
    }

    private static void ConfigureRemoteWindow(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        string? resolvedToken = null;
        try
        {
            var remote = ParseRemoteArguments(desktop.Args)
                ?? throw new InvalidDataException(
                    "usage: --remote HTTPS_ORIGIN --remote-ca CA_PATH [--remote-cache CACHE_PATH]");
            var endpoint = RemoteClientOptions.ParseEndpoint(remote.Endpoint);
            var token = RemoteTokenResolver.Resolve(endpoint);
            resolvedToken = token.Value;
            var options = RemoteClientOptions.Create(
                remote.Endpoint,
                Path.GetFullPath(remote.Certificate),
                token.Value,
                remote.Cache is null ? null : Path.GetFullPath(remote.Cache));
            desktop.MainWindow = new RemoteMainWindow(options, token.Source);
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var description = StartupFailure.Describe(
                error,
                resolvedToken,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            Console.Error.WriteLine($"Leserpent remote startup failed: {description}");
            var window = new StartupErrorWindow(description);
            window.Closed += (_, _) => desktop.Shutdown(StartupFailure.ExitCode);
            desktop.MainWindow = window;
        }
    }

    private static RemoteArguments? ParseRemoteArguments(string[]? args) => args switch
    {
        ["--remote", var endpoint, "--remote-ca", var certificate] =>
            new RemoteArguments(endpoint, certificate, null),
        ["--remote", var endpoint, "--remote-ca", var certificate,
            "--remote-cache", var cache] =>
            new RemoteArguments(endpoint, certificate, cache),
        _ => null,
    };

    private static RendererFixture LoadFixture(
        string[]? args,
        bool verifyControls,
        bool verifyFocusRetention)
    {
        var fixturePath = args switch
        {
            [var path] when !verifyControls && !verifyFocusRetention => path,
            ["--verify-controls", var path] when verifyControls => path,
            ["--verify-focus-retention", var path] when verifyFocusRetention => path,
            _ => throw new InvalidDataException(
                "usage: Leserpent.Avalonia [--verify-controls|--verify-focus-retention] FIXTURE | --verify-startup-error | --remote HTTPS_ORIGIN --remote-ca CA_PATH [--remote-cache CACHE_PATH]"),
        };
        if (string.IsNullOrWhiteSpace(fixturePath))
        {
            throw new InvalidDataException("fixture path is empty");
        }

        using var stream = new FileStream(
            fixturePath,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read);
        if (stream.Length > MaxPayloadBytes)
        {
            throw new InvalidDataException("fixture exceeds the UI IR payload limit");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        if (stream.ReadByte() != -1)
        {
            throw new InvalidDataException("fixture changed while being read");
        }

        var fixture = JsonSerializer.Deserialize(
            payload,
            RendererJsonContext.Default.RendererFixture)
            ?? throw new InvalidDataException("fixture is empty");
        if (fixture.SchemaVersion != 1)
        {
            throw new InvalidDataException("unsupported fixture schema");
        }

        return fixture;
    }

    private sealed record RemoteArguments(string Endpoint, string Certificate, string? Cache);
}
