using System.Text.Json;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Styling;
using Avalonia.Themes.Fluent;
using Avalonia.Threading;

internal sealed class LeserpentApp : Application
{
    private const int MaxPayloadBytes = 2 * 1024 * 1024;
    private static LocalOrchestraServiceSupervisor? localOrchestraService;
    private static readonly Dictionary<string, RemoteMainWindow> daemonSessions =
        new(StringComparer.Ordinal);
    private static bool shutdownHookInstalled;

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
            if (desktop.Args is ["--verify-desktop-connect-controls"])
            {
                ConfigureDesktopConnectionVerification(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is ["--verify-connection-management-controls"])
            {
                ConfigureConnectionManagementVerification(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is ["--verify-hub-topology"])
            {
                ConfigureHubTopologyVerification(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is ["--remote", ..])
            {
                ConfigureRemoteWindow(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is null or [])
            {
                ConfigureInteractiveDesktop(desktop);
                DesktopApplicationLifecycle.Configure(
                    this,
                    desktop,
                    () =>
                    {
                        ConfigureInteractiveDesktop(desktop);
                        desktop.MainWindow?.Show();
                    },
                    () => ShowConnectionManager(desktop));
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
        RegisterMainWindowLifecycle(desktop, window);
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

    private static void ConfigureDesktopConnectionVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var fixtureToken = new string('t', 32);
        DesktopConnectionRequest? submitted = null;
        var testCount = 0;
        var window = new DesktopConnectionWindow(null, null, request =>
        {
            submitted = request;
            return "verification only";
        }, (_, _) =>
        {
            testCount++;
            return Task.FromResult<string?>(null);
        });
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += async (_, _) =>
        {
            window.VerifyAccessibility();
            await window.ProbeConnectionTestAsync();
            if (testCount != 1)
            {
                throw new InvalidDataException(
                    "desktop connection test was not invoked exactly once");
            }
            window.ProbeSecureTokenSubmission(fixtureToken);
            if (submitted?.Token != fixtureToken)
            {
                throw new InvalidDataException(
                    "desktop connection did not submit the protected token");
            }
            Console.WriteLine(
                "desktop connection controls valid: controls=9, automation_ids=9, automation_names=9, live_region=true, token_input=secure, token_cleared=true, connection_test=true, test_side_effects=false");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
    }

    private static void ConfigureConnectionManagementVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var profile = new DesktopConnectionProfile
        {
            SchemaVersion = 1,
            Endpoint = "https://control.example:9443",
            CertificateAuthorityPath = "/verification/ca.pem",
        };
        var window = new DesktopConnectionWindow(
            profile,
            null,
            _ => "verification only",
            (_, _) => Task.FromResult<string?>(null),
            null,
            () => "verification only");
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += (_, _) =>
        {
            window.VerifyAccessibility();
            new DesktopForgetConnectionWindow(profile.Endpoint, () => null)
                .VerifyAccessibility();
            Console.WriteLine(
                "desktop connection management controls valid: settings_controls=10, confirmation_controls=3, automation_ids=true, automation_names=true, forget_confirmation=true, endpoint_scoped=true, connection_test=true");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
    }

    private static void ConfigureHubTopologyVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var topology = new RemoteTopologySnapshot(7,
        [
            new RemoteRuntimeProjection
            {
                Id = "runtime-a",
                Name = "Runtime A",
                Revision = 7,
                RefreshStatus = RefreshStatus.Ready,
                Tags = new RuntimeTags { Environment = "production" },
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            },
            new RemoteRuntimeProjection
            {
                Id = "runtime-b",
                Name = "Runtime B",
                Revision = 7,
                RefreshStatus = RefreshStatus.Pending,
                Tags = new RuntimeTags { Environment = "staging" },
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            },
        ]);
        var connections = new[]
        {
            DesktopDaemonConnection.FromProfile(new DesktopConnectionProfile
            {
                SchemaVersion = 1,
                Endpoint = "https://alpha.example:9443",
                CertificateAuthorityPath = "/verification/alpha-ca.pem",
            }),
            DesktopDaemonConnection.FromProfile(new DesktopConnectionProfile
            {
                SchemaVersion = 1,
                Endpoint = "https://beta.example:9443",
                CertificateAuthorityPath = "/verification/beta-ca.pem",
            }),
        };
        var window = new HubWindow(
            connections,
            true,
            null,
            () => "verification only",
            _ => "verification only",
            _ => Task.FromResult(topology),
            (_, _) => Task.FromResult(topology),
            () => { },
            _ => { });
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += (_, _) =>
        {
            DispatcherTimer.RunOnce(() =>
            {
                window.VerifyTopologyContract();
                if (window.RenderedRuntimeCount != 6)
                {
                    throw new InvalidDataException(
                        "Hub topology did not render daemon-owned runtime children");
                }
                Console.WriteLine(
                    "Hub topology valid: client_root=true, local_daemon=true, remote_daemons=2, runtime_children=6, bounded_preview=true, independent_actions=true, legacy_remote_button=false, automation=true");
                window.Close();
            }, TimeSpan.FromMilliseconds(150));
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
            var window = new RemoteMainWindow(options, token.Source);
            RegisterMainWindowLifecycle(desktop, window);
            desktop.MainWindow = window;
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var description = StartupFailure.Describe(
                error,
                resolvedToken,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            Console.Error.WriteLine($"Leserpent remote startup failed: {description}");
            var window = new StartupErrorWindow(description);
            RegisterMainWindowLifecycle(desktop, window);
            window.Closed += (_, _) => desktop.Shutdown(StartupFailure.ExitCode);
            desktop.MainWindow = window;
        }
    }

    private static void ConfigureInteractiveDesktop(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var legacyStore = new DesktopConnectionProfileStore(
            DesktopConnectionProfileStore.DefaultPath());
        var catalogStore = new DesktopConnectionCatalogStore(
            DesktopConnectionCatalogStore.DefaultPath());
        var certificateStore = DesktopCertificateAuthorityStore.Default();
        var catalog = DesktopConnectionCatalog.Empty;
        string? initialError = null;
        try
        {
            catalog = catalogStore.LoadOrMigrate(legacyStore);
            catalog = DesktopProductStartup.PrepareSavedCatalog(
                catalog,
                catalogStore,
                certificateStore);
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            catalog = DesktopConnectionCatalog.Empty;
            initialError = StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
        }

        if (localOrchestraService is null && !IsMobilePlatform())
        {
            localOrchestraService = new LocalOrchestraServiceSupervisor();
            localOrchestraService.TryEnsureReady(
                certificateStore,
                out _,
                out var localStartupError);
            if (!shutdownHookInstalled)
            {
                AppDomain.CurrentDomain.ProcessExit += (_, _) => localOrchestraService?.Dispose();
                shutdownHookInstalled = true;
            }
            if (initialError is null && localStartupError is not null)
            {
                initialError = localStartupError;
            }
            else if (initialError is not null && localStartupError is not null)
            {
                initialError = $"{localStartupError}{Environment.NewLine}{initialError}";
            }
        }

        try
        {
            certificateStore.PruneExcept(RetainedCertificatePaths(catalog));
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var trustError = StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            initialError = string.IsNullOrWhiteSpace(initialError)
                ? trustError
                : $"{initialError}{Environment.NewLine}{trustError}";
        }

        var hub = new HubWindow(
            catalog.Connections,
            !IsMobilePlatform(),
            initialError,
            () => OpenLocalOrchestra(desktop, certificateStore),
            connection => OpenRemoteFromConnection(
                desktop,
                catalogStore,
                certificateStore,
                connection),
            cancellationToken => LoadLocalTopologyAsync(
                certificateStore,
                cancellationToken),
            (connection, cancellationToken) => LoadRemoteTopologyAsync(
                catalogStore,
                certificateStore,
                connection,
                cancellationToken),
            () => ShowConnectionManager(desktop, null),
            connection => ShowConnectionManager(desktop, connection));
        RegisterMainWindowLifecycle(desktop, hub);
        desktop.MainWindow = hub;
    }

    private static string? OpenRemoteFromConnection(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        DesktopDaemonConnection expected)
    {
        try
        {
            var saved = catalogStore.Load().Connections.SingleOrDefault(
                item => item.DaemonId == expected.DaemonId);
            if (saved is null || saved != expected)
            {
                return "This daemon connection changed. Reopen the Hub before connecting.";
            }
            var catalog = DesktopProductStartup.PrepareSavedCatalog(
                catalogStore.Load(),
                catalogStore,
                certificateStore);
            saved = catalog.Connections.Single(item => item.DaemonId == expected.DaemonId);
            var plan = DesktopProductStartup.Resolve(saved.Profile);
            OpenProductRemoteWindow(desktop, plan);
            return null;
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            return StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
        }
    }

    private static string? OpenLocalOrchestra(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopCertificateAuthorityStore certificateStore)
    {
        if (IsMobilePlatform())
        {
            return "Self-host is not supported on this platform.";
        }
        if (localOrchestraService is null)
        {
            localOrchestraService = new LocalOrchestraServiceSupervisor();
        }
        try
        {
            if (localOrchestraService.TryEnsureReady(
                    certificateStore,
                    out var localPlan,
                    out var startupError)
                && localPlan is not null)
            {
                OpenProductRemoteWindow(desktop, localPlan);
                return null;
            }
            return startupError ?? "Local orchestra did not become ready.";
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            return StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
        }
    }

    private static async Task<RemoteTopologySnapshot> LoadLocalTopologyAsync(
        DesktopCertificateAuthorityStore certificateStore,
        CancellationToken cancellationToken)
    {
        localOrchestraService ??= new LocalOrchestraServiceSupervisor();
        var plan = await Task.Run(() =>
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (!localOrchestraService.TryEnsureReady(
                    certificateStore,
                    out var localPlan,
                    out var startupError)
                || localPlan is null)
            {
                throw new InvalidDataException(
                    startupError ?? "local orchestra did not become ready");
            }
            return localPlan;
        }, cancellationToken);
        using var client = new RemoteTopologyClient(plan.Options);
        return await client.LoadAsync("avalonia-hub", cancellationToken);
    }

    private static async Task<RemoteTopologySnapshot> LoadRemoteTopologyAsync(
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        DesktopDaemonConnection expected,
        CancellationToken cancellationToken)
    {
        var catalog = catalogStore.Load();
        var current = catalog.Connections.SingleOrDefault(
            item => item.DaemonId == expected.DaemonId);
        if (current is null || current != expected)
        {
            throw new InvalidDataException(
                "the saved daemon connection changed; reopen the Hub before refreshing it");
        }
        try
        {
            catalog = DesktopProductStartup.PrepareSavedCatalog(
                catalog,
                catalogStore,
                certificateStore);
            current = catalog.Connections.Single(item => item.DaemonId == expected.DaemonId);
            var plan = DesktopProductStartup.Resolve(current.Profile);
            using var client = new RemoteTopologyClient(plan.Options);
            return await client.LoadAsync("avalonia-hub", cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception liveError) when (StartupFailure.IsExpected(liveError)
            || liveError is TaskCanceledException or TimeoutException)
        {
            try
            {
                var endpoint = RemoteClientOptions.ParseEndpoint(current.Profile.Endpoint);
                var cache = new RemoteSnapshotStore(
                    endpoint,
                    RemoteSnapshotStore.DefaultPath(endpoint)).Load();
                if (cache is not null)
                {
                    return RemoteTopologyCodec.FromCache(cache);
                }
            }
            catch (Exception cacheError) when (StartupFailure.IsExpected(cacheError))
            {
                throw new InvalidDataException(
                    "live daemon topology and its cached snapshot are unavailable",
                    cacheError);
            }
            throw new InvalidDataException(
                "live daemon topology is unavailable and has no cached snapshot",
                liveError);
        }
    }

    private static bool IsMobilePlatform() => OperatingSystem.IsIOS()
        || OperatingSystem.IsAndroid();

    private static void ShowConnectionManager(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopDaemonConnection? connection = null)
    {
        var existing = desktop.Windows.OfType<DesktopConnectionWindow>().FirstOrDefault();
        if (existing is not null)
        {
            existing.Show();
            existing.Activate();
            return;
        }

        string? initialError = null;
        ShowConnectionWindow(
            desktop,
            new DesktopConnectionCatalogStore(DesktopConnectionCatalogStore.DefaultPath()),
            connection,
            initialError,
            false);
    }

    private static void ShowConnectionWindow(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopConnectionCatalogStore catalogStore,
        DesktopDaemonConnection? connection,
        string? initialError,
        bool isInitialSetup)
    {
        var previousMainWindow = desktop.MainWindow;
        var certificateStore = DesktopCertificateAuthorityStore.Default();
        DesktopConnectionWindow? setup = null;
        setup = new DesktopConnectionWindow(
            connection?.Profile,
            initialError,
            request =>
            {
                try
                {
                    var requestedProfile = new DesktopConnectionProfile
                    {
                        SchemaVersion = 1,
                        Endpoint = request.Endpoint,
                        CertificateAuthorityPath = request.Remember
                            ? certificateStore.Import(request.CertificateAuthorityPath)
                            : Path.GetFullPath(request.CertificateAuthorityPath),
                    };
                    var plan = DesktopProductStartup.Resolve(
                        requestedProfile,
                        request.Token);
                    if (request.Remember)
                    {
                        var savedConnection = DesktopDaemonConnection.FromProfile(
                            requestedProfile);
                        catalogStore.Upsert(savedConnection, connection?.DaemonId);
                    }
                    certificateStore.PruneExcept(RetainedCertificatePaths(
                        catalogStore.Load()));
                    OpenProductRemoteWindow(desktop, plan);
                    setup!.Close();
                    if (request.Remember)
                    {
                        RefreshHub(desktop);
                    }
                    return null;
                }
                catch (Exception error) when (StartupFailure.IsExpected(error))
                {
                    return StartupFailure.Describe(
                        error,
                        Environment.GetEnvironmentVariable(
                            RemoteTokenResolver.EnvironmentVariable));
                }
            },
            TestConnectionAsync,
            isInitialSetup ? () => desktop.TryShutdown(0) : null,
            connection is null
                ? null
                : () =>
                {
                    var error = ForgetSavedConnection(
                        connection,
                        catalogStore,
                        certificateStore);
                    if (error is null)
                    {
                        DispatcherTimer.RunOnce(
                            () => RefreshHub(desktop),
                            TimeSpan.FromMilliseconds(100));
                    }
                    return error;
                });
        if (isInitialSetup)
        {
            RegisterMainWindowLifecycle(desktop, setup);
            desktop.MainWindow = setup;
            return;
        }
        if (previousMainWindow is not null)
        {
            setup.Show(previousMainWindow);
        }
        else
        {
            setup.Show();
        }
    }

    private static RemoteMainWindow CreateProductRemoteWindow(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopProductStartupPlan plan) =>
        new(
            plan.Options,
            plan.TokenSource,
            () => ShowConnectionManager(desktop, FindSavedConnection(plan.Profile)));

    private static void OpenProductRemoteWindow(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopProductStartupPlan plan)
    {
        var sessionKey = RemoteClientOptions.ParseEndpoint(plan.Profile.Endpoint).ToString();
        if (daemonSessions.TryGetValue(sessionKey, out var existing))
        {
            existing.Show();
            existing.Activate();
            return;
        }
        var remote = CreateProductRemoteWindow(desktop, plan);
        RegisterMainWindowLifecycle(desktop, remote);
        daemonSessions.Add(sessionKey, remote);
        remote.Closed += (_, _) => daemonSessions.Remove(sessionKey);
        remote.Show();
    }

    private static DesktopDaemonConnection? FindSavedConnection(
        DesktopConnectionProfile profile)
    {
        try
        {
            var daemonId = DesktopDaemonConnection.DeriveDaemonId(profile.Endpoint);
            return new DesktopConnectionCatalogStore(
                DesktopConnectionCatalogStore.DefaultPath())
                .Load()
                .Connections
                .SingleOrDefault(item => item.DaemonId == daemonId);
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            return null;
        }
    }

    private static void RefreshHub(IClassicDesktopStyleApplicationLifetime desktop)
    {
        var previousHub = desktop.MainWindow as HubWindow;
        ConfigureInteractiveDesktop(desktop);
        var currentHub = desktop.MainWindow;
        currentHub?.Show();
        currentHub?.Activate();
        if (previousHub is not null && !ReferenceEquals(previousHub, currentHub))
        {
            previousHub.Close();
        }
    }

    private static async Task<string?> TestConnectionAsync(
        DesktopConnectionRequest request,
        CancellationToken cancellationToken)
    {
        try
        {
            var endpoint = RemoteClientOptions.ParseEndpoint(request.Endpoint);
            var token = request.Token ?? RemoteTokenResolver.Resolve(endpoint).Value;
            var options = RemoteClientOptions.Create(
                request.Endpoint,
                Path.GetFullPath(request.CertificateAuthorityPath),
                token);
            using var client = new RemoteHealthClient(options);
            await client.CheckAsync(cancellationToken);
            return null;
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            return StartupFailure.Describe(
                error,
                request.Token,
                Environment.GetEnvironmentVariable(
                    RemoteTokenResolver.EnvironmentVariable));
        }
    }

    private static string? ForgetSavedConnection(
        DesktopDaemonConnection connection,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore)
    {
        try
        {
            var current = catalogStore.Load().Connections.SingleOrDefault(
                item => item.DaemonId == connection.DaemonId);
            if (current != connection)
            {
                throw new InvalidDataException(
                    "the saved daemon connection changed; reopen the Hub before removing it");
            }
            var endpoint = RemoteClientOptions.ParseEndpoint(connection.Profile.Endpoint);
            RemoteTokenResolver.Delete(endpoint);
            var catalog = catalogStore.Remove(connection);
            certificateStore.PruneExcept(RetainedCertificatePaths(catalog));
            return null;
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            return StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
        }
    }

    private static IEnumerable<string> RetainedCertificatePaths(
        DesktopConnectionCatalog catalog)
    {
        foreach (var connection in catalog.Connections)
        {
            yield return connection.Profile.CertificateAuthorityPath;
        }
        if (localOrchestraService?.ManagedAuthorityPath is { } localAuthority)
        {
            yield return localAuthority;
        }
    }

    private static void RegisterMainWindowLifecycle(
        IClassicDesktopStyleApplicationLifetime desktop,
        Window window)
    {
        window.Closed += (_, _) =>
        {
            if (ReferenceEquals(desktop.MainWindow, window))
            {
                desktop.MainWindow = null;
            }
        };
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
