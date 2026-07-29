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
            if (desktop.Args is ["--verify-bootstrap-controls"])
            {
                ConfigureBootstrapControlVerification(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is ["--verify-provisioning-controls"])
            {
                ConfigureProvisioningControlVerification(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is ["--verify-retirement-controls"])
            {
                ConfigureRetirementControlVerification(desktop);
                base.OnFrameworkInitializationCompleted();
                return;
            }
            if (desktop.Args is ["--verify-daemon-retirement-controls"])
            {
                ConfigureDaemonRetirementControlVerification(desktop);
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
                window.Opened += async (_, _) =>
                {
                    try
                    {
                        await window.CompleteInitialWaitProbesAsync();
                        var nodeId = window.BeginFocusRetentionProbe();
                        DispatcherTimer.RunOnce(
                            () =>
                            {
                                try
                                {
                                    window.CompleteFocusRetentionProbe(nodeId);
                                    window.BeginPatchedFocusRetentionProbe(nodeId);
                                    DispatcherTimer.RunOnce(
                                        () =>
                                        {
                                            try
                                            {
                                                window.CompleteFocusRetentionProbe(nodeId);
                                                window.ProbeRemovedFocusTarget(nodeId);
                                                Console.WriteLine(
                                                    $"Avalonia focus retention valid: node={nodeId}, leselang_presentation=true, navigate_focus=true, navigate_focus_forward={window.FocusNavigationForwardCompleted.ToString().ToLowerInvariant()}, navigate_focus_backward={window.FocusNavigationBackwardCompleted.ToString().ToLowerInvariant()}, navigate_focus_first={window.FocusNavigationFirstCompleted.ToString().ToLowerInvariant()}, navigate_focus_last={window.FocusNavigationLastCompleted.ToString().ToLowerInvariant()}, navigate_focus_stable_destination=true, navigate_focus_failure_preserved_focus={window.FocusNavigationFailuresPreservedFocus.ToString().ToLowerInvariant()}, navigate_focus_no_activation={window.FocusNavigationDidNotActivate.ToString().ToLowerInvariant()}, scroll_into_view=true, scroll_focus_preserved=true, assert_visible=true, assert_hidden={window.HiddenAssertCompleted.ToString().ToLowerInvariant()}, wait_hidden={window.InitialHiddenWaitCompleted.ToString().ToLowerInvariant()}, wait_hidden_external_transition={window.InitialHiddenWaitCompleted.ToString().ToLowerInvariant()}, wait_hidden_timeout={window.InitialHiddenWaitTimedOut.ToString().ToLowerInvariant()}, assert_realized=true, wait_realized=true, wait_realized_natural_layout=true, wait_realized_timeout=true, wait_visible=true, wait_visible_natural_layout=true, wait_visible_timeout=true, wait_enabled=true, wait_enabled_external_transition=true, wait_enabled_timeout=true, wait_disabled={window.InitialDisabledWaitCompleted.ToString().ToLowerInvariant()}, wait_disabled_external_transition={window.InitialDisabledWaitCompleted.ToString().ToLowerInvariant()}, wait_disabled_timeout={window.InitialDisabledWaitTimedOut.ToString().ToLowerInvariant()}, assert_window_open={window.WindowOpenAssertCompleted.ToString().ToLowerInvariant()}, wait_focused=true, wait_focused_external_transition=true, wait_focused_timeout=true, wait_focused_no_focus_mutation=true, assert_selection={window.SelectionAssertCompleted.ToString().ToLowerInvariant()}, wait_selection={window.InitialSelectionWaitCompleted.ToString().ToLowerInvariant()}, wait_selection_timeout={window.InitialSelectionWaitTimedOut.ToString().ToLowerInvariant()}, selection_mismatch_rejected={window.SelectionMismatchRejected.ToString().ToLowerInvariant()}, selectionless_target_rejected={window.SelectionlessTargetRejected.ToString().ToLowerInvariant()}, selection_focus_preserved={window.SelectionProbePreservedFocus.ToString().ToLowerInvariant()}, assert_focused=true, assert_enabled=true, assert_disabled={window.DisabledAssertCompleted.ToString().ToLowerInvariant()}, assert_text=true, assert_automation_id=true, assert_node_kind=true, assert_action_kind={window.ActionKindAssertCompleted.ToString().ToLowerInvariant()}, assert_accessible_name=true, assert_accessible_description=true, unrealized_target_rejected=true, text_mismatch_rejected=true, automation_id_mismatch_rejected=true, node_kind_mismatch_rejected=true, action_kind_mismatch_rejected={window.ActionKindMismatchRejected.ToString().ToLowerInvariant()}, accessible_name_mismatch_rejected=true, accessible_description_mismatch_rejected=true, disabled_target_rejected=true, enabled_target_disabled_assertion_rejected={window.DisabledMismatchRejected.ToString().ToLowerInvariant()}, unfocused_target_rejected=true, hidden_target_rejected=true, visible_target_hidden_assertion_rejected={window.VisibleMismatchRejected.ToString().ToLowerInvariant()}, missing_target_rejected=true, unfocusable_target_rejected=true, remount=true, patch_update=true, restored=true, removed_target_safe=true");
                                                desktop.Shutdown(0);
                                            }
                                            catch (Exception error)
                                            {
                                                ReportVerificationFailure(
                                                    desktop,
                                                    "focus retention remount probe",
                                                    error);
                                            }
                                        },
                                        TimeSpan.FromMilliseconds(200));
                                }
                                catch (Exception error)
                                {
                                    ReportVerificationFailure(
                                        desktop,
                                        "focus retention patch probe",
                                        error);
                                }
                            },
                            TimeSpan.FromMilliseconds(200));
                    }
                    catch (Exception error)
                    {
                        ReportVerificationFailure(
                            desktop,
                            "focus retention initial probe",
                            error);
                    }
                };
            }
        }
        base.OnFrameworkInitializationCompleted();
    }

    private static void ReportVerificationFailure(
        IClassicDesktopStyleApplicationLifetime desktop,
        string phase,
        Exception error)
    {
        Console.Error.WriteLine(
            $"Leserpent verification failed during {phase}: {error.Message}");
        desktop.Shutdown(StartupFailure.ExitCode);
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
            var trustProfile = new DesktopConnectionProfile
            {
                SchemaVersion = 1,
                Endpoint = profile.Endpoint,
                BootstrapTrustRoot = "/verification/bootstrap-trust",
                BootstrapTrustHandle = "vault:leserpent-ca:control-example",
            };
            DesktopConnectionRequest? trustSubmission = null;
            var trustWindow = new DesktopConnectionWindow(
                trustProfile,
                null,
                request =>
                {
                    trustSubmission = request;
                    return "verification only";
                },
                (_, _) => Task.FromResult<string?>(null));
            trustWindow.VerifyAccessibility();
            trustWindow.ProbeSecureTokenSubmission(new string('u', 32));
            if (trustSubmission?.CertificateAuthorityPath.Length != 0
                || trustSubmission.BootstrapTrustRoot != trustProfile.BootstrapTrustRoot
                || trustSubmission.BootstrapTrustHandle != trustProfile.BootstrapTrustHandle)
            {
                throw new InvalidDataException(
                    "desktop connection settings did not retain bootstrap trust authority");
            }
            trustWindow.Close();
            Console.WriteLine(
                "desktop connection management controls valid: settings_controls=10, confirmation_controls=3, automation_ids=true, automation_names=true, forget_confirmation=true, endpoint_scoped=true, connection_test=true, bootstrap_trust_retained=true");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
    }

    private static void ConfigureHubTopologyVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var runtimeOpenCount = 0;
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
        ], Health: new RemoteHealth(
            "ready",
            true,
            1,
            new RemoteEffectQueueHealth(2, 1, 4, 0, 3, 4, 16, false)));
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
            (_, _) =>
            {
                runtimeOpenCount++;
                return null;
            },
            (_, _, _) =>
            {
                runtimeOpenCount++;
                return null;
            },
            _ => Task.FromResult(topology),
            (_, _) => Task.FromResult(topology),
            () => { },
            () => { },
            () => { },
            () => { },
            () => { },
            _ => { });
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += (_, _) =>
        {
            DispatcherTimer.RunOnce(() =>
            {
                window.VerifyTopologyContract();
                RemoteWorkspaceLaunchPolicy.VerifyContract();
                if (window.RenderedRuntimeCount != 6
                    || window.RenderedRuntimeActionCount != 6
                    || window.LiveTopologyCount != 3
                    || window.VerifiedAuthorityCount != 3)
                {
                    throw new InvalidDataException(
                        "Hub topology did not render live actionable daemon-owned runtime children");
                }
                window.ProbeFirstRuntimeAction();
                if (runtimeOpenCount != 1)
                {
                    throw new InvalidDataException(
                        "Hub runtime action did not preserve its daemon route");
                }
                Console.WriteLine(
                    "Hub topology valid: client_root=true, local_daemon=true, remote_daemons=2, live_topologies=3, authority_proofs=3, queue_health=true, runtime_children=6, runtime_actions=6, daemon_route=true, authoritative_workspace_gate=true, retained_topology_state=true, revision_regression_fence=true, bounded_auto_refresh=true, bounded_preview=true, independent_actions=true, legacy_remote_button=false, automation=true");
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

    private static void ConfigureBootstrapControlVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var submitCount = 0;
        var inspectCount = 0;
        var bindCount = 0;
        var promoteCount = 0;
        var target = new RemoteBootstrapSnapshot(
            "bootstrap-verification",
            "planned",
            "ssh",
            "target.example",
            22,
            true,
            null,
            null,
            null,
            null,
            null,
            false);
        var window = new BootstrapDeploymentWindow(
            [new BootstrapAuthorityOption(
                "daemon-verification",
                "Verification authority",
                "https://controller.example:9443",
                true)],
            new BootstrapHubOperations(
                (_, intent, _) =>
                {
                    submitCount++;
                    target = target with { BootstrapId = intent.BootstrapId };
                    return Task.FromResult(target);
                },
                (_, bootstrapId, _, _) =>
                {
                    inspectCount++;
                    target = target with
                    {
                        BootstrapId = bootstrapId,
                        Phase = "bootstrapped",
                        DaemonId = "daemon-target",
                        Endpoint = "https://target.example:9443",
                        SessionCredentialHandle = "vault:leserpentd:target",
                        TrustCredentialHandle = "vault:leserpent-ca:target",
                    };
                    return Task.FromResult(target);
                },
                (_, bootstrapId, _, _) =>
                {
                    bindCount++;
                    target = target with
                    {
                        BootstrapId = bootstrapId,
                        Phase = "session_bound",
                        BootstrapCredentialPresent = false,
                        MutationAuthorized = true,
                    };
                    return Task.FromResult(target);
                },
                (_, state, _) =>
                {
                    if (state is not { Phase: "session_bound", MutationAuthorized: true })
                    {
                        throw new InvalidDataException(
                            "bootstrap controls promoted an unbound session");
                    }
                    promoteCount++;
                    return Task.CompletedTask;
                }));
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += async (_, _) =>
        {
            window.VerifyAccessibility();
            await window.ProbeConfirmationFenceAsync();
            if (submitCount != 0)
            {
                throw new InvalidDataException(
                    "bootstrap controls submitted without explicit confirmation");
            }
            await window.ProbeWorkflowAsync();
            if (submitCount != 1 || inspectCount != 1 || bindCount != 1 || promoteCount != 1)
            {
                throw new InvalidDataException(
                    "bootstrap controls did not preserve the submit-inspect-bind-promote sequence");
            }
            Console.WriteLine(
                "bootstrap controls valid: controls=12, authority_scoped=true, opaque_ssh_handle=true, explicit_confirmation=true, unconfirmed_submit_blocked=true, submit=true, inspect=true, bind=true, phase_gated=true, polling=true, mutation_authorized=true, local_promotion=true, automation=true");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
    }

    private static void ConfigureProvisioningControlVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var reconcileCount = 0;
        RemoteProvisioningIntent? acceptedIntent = null;
        var window = new GewyvernProvisioningWindow(
            [new BootstrapAuthorityOption(
                "daemon-verification",
                "Verification authority",
                "https://controller.example:9443",
                false)],
            new ProvisioningHubOperations((_, intent, _) =>
            {
                reconcileCount++;
                acceptedIntent ??= intent;
                if (acceptedIntent != intent)
                {
                    throw new InvalidDataException(
                        "provisioning controls changed identity while observing progress");
                }
                return Task.FromResult(reconcileCount == 1
                    ? new RemoteProvisioningSnapshot(
                        intent.ProvisioningId,
                        intent.RuntimeId,
                        "planned",
                        "ssh",
                        intent.Host,
                        intent.Port,
                        true,
                        null,
                        null,
                        null,
                        null,
                        false)
                    : new RemoteProvisioningSnapshot(
                        intent.ProvisioningId,
                        intent.RuntimeId,
                        "runtime_registered",
                        "ssh",
                        intent.Host,
                        intent.Port,
                        false,
                        "https://runtime.example:9444",
                        "vault:gewyvern:runtime-api",
                        "vault:gewyvern-ca:runtime-ca",
                        null,
                        true));
            }));
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += async (_, _) =>
        {
            window.VerifyAccessibility();
            await window.ProbeConfirmationFenceAsync();
            if (reconcileCount != 0)
            {
                throw new InvalidDataException(
                    "provisioning controls submitted without explicit confirmation");
            }
            await window.ProbeWorkflowAsync();
            if (reconcileCount != 2 || acceptedIntent is null)
            {
                throw new InvalidDataException(
                    "provisioning controls did not preserve submit-observe identity");
            }
            Console.WriteLine(
                "provisioning controls valid: controls=12, authority_scoped=true, opaque_ssh_handle=true, explicit_confirmation=true, unconfirmed_submit_blocked=true, stable_identity=true, bounded_polling=30, terminal_state=true, retry_guidance=true, automation=true");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
    }

    private static void ConfigureRetirementControlVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var reconcileCount = 0;
        RemoteRetirementIntent? acceptedIntent = null;
        var window = new GewyvernRetirementWindow(
            [new BootstrapAuthorityOption(
                "daemon-verification",
                "Verification authority",
                "https://controller.example:9443",
                false)],
            new RetirementHubOperations((_, intent, _) =>
            {
                reconcileCount++;
                acceptedIntent ??= intent;
                if (acceptedIntent != intent)
                {
                    throw new InvalidDataException(
                        "retirement controls changed identity while observing progress");
                }
                return Task.FromResult(reconcileCount == 1
                    ? new RemoteRetirementSnapshot(
                        intent.RetirementId,
                        intent.ProvisioningId,
                        intent.RuntimeId,
                        "planned",
                        "ssh",
                        intent.Host,
                        intent.Port,
                        true,
                        false,
                        true,
                        null)
                    : new RemoteRetirementSnapshot(
                        intent.RetirementId,
                        intent.ProvisioningId,
                        intent.RuntimeId,
                        "runtime_unregistered",
                        "ssh",
                        intent.Host,
                        intent.Port,
                        false,
                        true,
                        false,
                        null));
            }));
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += async (_, _) =>
        {
            window.VerifyAccessibility();
            await window.ProbeConfirmationFenceAsync();
            if (reconcileCount != 0)
            {
                throw new InvalidDataException(
                    "retirement controls submitted without explicit confirmation");
            }
            await window.ProbeWorkflowAsync();
            if (reconcileCount != 2 || acceptedIntent is null)
            {
                throw new InvalidDataException(
                    "retirement controls did not preserve submit-observe identity");
            }
            Console.WriteLine(
                "retirement controls valid: controls=13, authority_scoped=true, provisioning_bound=true, opaque_ssh_handle=true, explicit_confirmation=true, unconfirmed_submit_blocked=true, stable_identity=true, bounded_polling=30, terminal_state=true, failure_preserves_registration=true, retry_guidance=true, automation=true");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
    }

    private static void ConfigureDaemonRetirementControlVerification(
        IClassicDesktopStyleApplicationLifetime desktop)
    {
        var reconcileCount = 0;
        RemoteDaemonRetirementIntent? acceptedIntent = null;
        var window = new DaemonRetirementWindow(
            [new BootstrapAuthorityOption(
                "daemon-verification",
                "Verification authority",
                "https://controller.example:9443",
                false)],
            new DaemonRetirementHubOperations((_, intent, _) =>
            {
                reconcileCount++;
                acceptedIntent ??= intent;
                if (acceptedIntent != intent)
                {
                    throw new InvalidDataException(
                        "daemon retirement controls changed identity while observing progress");
                }
                return Task.FromResult(reconcileCount == 1
                    ? new RemoteDaemonRetirementSnapshot(
                        intent.RetirementId,
                        intent.BootstrapId,
                        "daemon-target",
                        "planned",
                        "ssh",
                        "daemon.example",
                        22,
                        new string('a', 64),
                        "system",
                        true,
                        false,
                        null)
                    : new RemoteDaemonRetirementSnapshot(
                        intent.RetirementId,
                        intent.BootstrapId,
                        "daemon-target",
                        "service_retired",
                        "ssh",
                        "daemon.example",
                        22,
                        new string('a', 64),
                        "system",
                        false,
                        true,
                        null));
            }));
        RegisterMainWindowLifecycle(desktop, window);
        window.Opened += async (_, _) =>
        {
            window.VerifyAccessibility();
            await window.ProbeConfirmationFenceAsync();
            if (reconcileCount != 0)
            {
                throw new InvalidDataException(
                    "daemon retirement controls submitted without explicit confirmation");
            }
            await window.ProbeWorkflowAsync();
            if (reconcileCount != 2 || acceptedIntent is null)
            {
                throw new InvalidDataException(
                    "daemon retirement controls did not preserve submit-observe identity");
            }
            Console.WriteLine(
                "daemon retirement controls valid: controls=10, authority_scoped=true, bootstrap_bound=true, authority_omitting=true, opaque_ssh_handle=true, explicit_confirmation=true, unconfirmed_submit_blocked=true, stable_identity=true, bounded_polling=30, terminal_state=true, retry_guidance=true, automation=true");
            DispatcherTimer.RunOnce(window.Close, TimeSpan.FromMilliseconds(100));
        };
        window.Closed += (_, _) => desktop.Shutdown(0);
        desktop.MainWindow = window;
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
            certificateStore.PruneExcept(RetainedCertificatePaths(catalog, certificateStore));
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
            (runtime, revision) => OpenLocalRuntimeWorkspace(
                desktop,
                certificateStore,
                runtime,
                revision),
            (connection, runtime, revision) => OpenRemoteRuntimeWorkspace(
                desktop,
                catalogStore,
                certificateStore,
                connection,
                runtime,
                revision),
            cancellationToken => LoadLocalTopologyAsync(
                certificateStore,
                cancellationToken),
            (connection, cancellationToken) => LoadRemoteTopologyAsync(
                catalogStore,
                certificateStore,
                connection,
                cancellationToken),
            () => ShowBootstrapDeployment(desktop, catalogStore, certificateStore),
            () => ShowDaemonRetirement(desktop, catalogStore, certificateStore),
            () => ShowGewyvernProvisioning(desktop, catalogStore, certificateStore),
            () => ShowGewyvernRetirement(desktop, catalogStore, certificateStore),
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
            var plan = ResolveRemoteConnectionPlan(
                catalogStore,
                certificateStore,
                expected);
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

    private static string? OpenRemoteRuntimeWorkspace(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        DesktopDaemonConnection connection,
        RemoteRuntimeProjection runtime,
        ulong topologyRevision)
    {
        try
        {
            var plan = ResolveRemoteConnectionPlan(
                catalogStore,
                certificateStore,
                connection);
            var session = OpenProductRemoteWindow(desktop, plan);
            return session.RequestRuntimeWorkspace(runtime.Id, topologyRevision);
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            return StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
        }
    }

    private static DesktopProductStartupPlan ResolveRemoteConnectionPlan(
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        DesktopDaemonConnection expected)
    {
        var saved = catalogStore.Load().Connections.SingleOrDefault(
            item => item.DaemonId == expected.DaemonId);
        if (saved is null || saved != expected)
        {
            throw new InvalidDataException(
                "this daemon connection changed; reopen the Hub before connecting");
        }
        var catalog = DesktopProductStartup.PrepareSavedCatalog(
            catalogStore.Load(),
            catalogStore,
            certificateStore);
        saved = catalog.Connections.Single(item => item.DaemonId == expected.DaemonId);
        return DesktopProductStartup.Resolve(saved.Profile, certificateStore);
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

    private static string? OpenLocalRuntimeWorkspace(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopCertificateAuthorityStore certificateStore,
        RemoteRuntimeProjection runtime,
        ulong topologyRevision)
    {
        if (IsMobilePlatform())
        {
            return "Self-host is not supported on this platform.";
        }
        localOrchestraService ??= new LocalOrchestraServiceSupervisor();
        try
        {
            if (!localOrchestraService.TryEnsureReady(
                    certificateStore,
                    out var plan,
                    out var startupError)
                || plan is null)
            {
                return startupError ?? "Local orchestra did not become ready.";
            }
            var session = OpenProductRemoteWindow(desktop, plan);
            return session.RequestRuntimeWorkspace(runtime.Id, topologyRevision);
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
        return await LoadLiveTopologyAsync(plan, cancellationToken);
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
            var plan = DesktopProductStartup.Resolve(current.Profile, certificateStore);
            return await LoadLiveTopologyAsync(plan, cancellationToken);
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

    private static async Task<RemoteTopologySnapshot> LoadLiveTopologyAsync(
        DesktopProductStartupPlan plan,
        CancellationToken cancellationToken)
    {
        using var topologyClient = new RemoteTopologyClient(plan.Options);
        using var healthClient = new RemoteHealthClient(plan.Options);
        var topologyTask = topologyClient.LoadAsync("avalonia-hub", cancellationToken);
        var healthTask = healthClient.CheckAsync(cancellationToken);
        await Task.WhenAll(topologyTask, healthTask);
        return (await topologyTask) with { Health = await healthTask };
    }

    private static bool IsMobilePlatform() => OperatingSystem.IsIOS()
        || OperatingSystem.IsAndroid();

    private static void ShowBootstrapDeployment(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore)
    {
        try
        {
            var catalog = DesktopProductStartup.PrepareSavedCatalog(
                catalogStore.Load(),
                catalogStore,
                certificateStore);
            var authorities = catalog.Connections
                .Select(connection => new BootstrapAuthorityOption(
                    connection.DaemonId,
                    connection.DisplayName,
                    connection.Profile.Endpoint,
                    false))
                .ToList();
            if (localOrchestraService is { BootstrapEnabled: true })
            {
                authorities.Insert(0, new BootstrapAuthorityOption(
                    "local-orchestra",
                    "Local Orchestra",
                    "Managed on this device",
                    true));
            }
            var operations = new BootstrapHubOperations(
                (authorityId, intent, cancellationToken) => ExecuteBootstrapAsync(
                    catalogStore,
                    certificateStore,
                    authorityId,
                    (client, token) => client.SubmitAsync(intent, token),
                    cancellationToken),
                (authorityId, bootstrapId, principal, cancellationToken) => ExecuteBootstrapAsync(
                    catalogStore,
                    certificateStore,
                    authorityId,
                    (client, token) => client.InspectAsync(bootstrapId, principal, token),
                    cancellationToken),
                (authorityId, bootstrapId, principal, cancellationToken) => ExecuteBootstrapAsync(
                    catalogStore,
                    certificateStore,
                    authorityId,
                    (client, token) => client.BindAsync(bootstrapId, principal, token),
                    cancellationToken),
                async (authorityId, state, cancellationToken) =>
                {
                    await PromoteBoundBootstrapAsync(
                        catalogStore,
                        certificateStore,
                        authorityId,
                        state,
                        cancellationToken);
                    RefreshHub(desktop);
                });
            var window = new BootstrapDeploymentWindow(authorities, operations);
            if (desktop.MainWindow is { } owner)
            {
                window.Show(owner);
            }
            else
            {
                window.Show();
            }
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var description = StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            new StartupErrorWindow(description).Show();
        }
    }

    private static async Task<RemoteBootstrapSnapshot> ExecuteBootstrapAsync(
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        string authorityId,
        Func<RemoteBootstrapClient, CancellationToken, Task<RemoteBootstrapSnapshot>> operation,
        CancellationToken cancellationToken)
    {
        if (authorityId == "local-orchestra")
        {
            if (localOrchestraService is not { BootstrapEnabled: true } localService)
            {
                throw new InvalidDataException("local bootstrap authority is unavailable");
            }
            if (!localService.TryEnsureReady(
                    certificateStore,
                    out var localPlan,
                    out var startupError)
                || localPlan is null)
            {
                throw new InvalidDataException(
                    startupError ?? "local bootstrap authority is unavailable");
            }
            using var localClient = new RemoteBootstrapClient(localPlan.Options);
            return await operation(localClient, cancellationToken);
        }
        var connection = catalogStore.Load().Connections.SingleOrDefault(
            item => item.DaemonId == authorityId)
            ?? throw new InvalidDataException(
                "the deployment authority changed; reopen the Hub before continuing");
        var plan = ResolveRemoteConnectionPlan(
            catalogStore,
            certificateStore,
            connection);
        using var client = new RemoteBootstrapClient(plan.Options);
        return await operation(client, cancellationToken);
    }

    private static void ShowDaemonRetirement(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore)
    {
        try
        {
            var catalog = DesktopProductStartup.PrepareSavedCatalog(
                catalogStore.Load(),
                catalogStore,
                certificateStore);
            var authorities = catalog.Connections
                .Select(connection => new BootstrapAuthorityOption(
                    connection.DaemonId,
                    connection.DisplayName,
                    connection.Profile.Endpoint,
                    false))
                .ToList();
            if (localOrchestraService is { BootstrapEnabled: true })
            {
                authorities.Insert(0, new BootstrapAuthorityOption(
                    "local-orchestra",
                    "Local Orchestra",
                    "Managed on this device",
                    false));
            }
            if (authorities.Count == 0)
            {
                throw new InvalidDataException(
                    "Add and authenticate the daemon authority that owns the original bootstrap before retiring its deployed daemon.");
            }
            var operations = new DaemonRetirementHubOperations(
                async (authorityId, intent, cancellationToken) =>
                {
                    var state = await ExecuteDaemonRetirementAsync(
                        catalogStore,
                        certificateStore,
                        authorityId,
                        intent,
                        cancellationToken);
                    if (state.ServiceRetired)
                    {
                        RefreshHub(desktop);
                    }
                    return state;
                });
            var window = new DaemonRetirementWindow(authorities, operations);
            if (desktop.MainWindow is { } owner)
            {
                window.Show(owner);
            }
            else
            {
                window.Show();
            }
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var description = StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            new StartupErrorWindow(description).Show();
        }
    }

    private static async Task<RemoteDaemonRetirementSnapshot> ExecuteDaemonRetirementAsync(
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        string authorityId,
        RemoteDaemonRetirementIntent intent,
        CancellationToken cancellationToken)
    {
        if (authorityId == "local-orchestra")
        {
            if (localOrchestraService is not { BootstrapEnabled: true } localService)
            {
                throw new InvalidDataException(
                    "local daemon retirement authority is unavailable");
            }
            if (!localService.TryEnsureReady(
                    certificateStore,
                    out var localPlan,
                    out var startupError)
                || localPlan is null)
            {
                throw new InvalidDataException(
                    startupError ?? "local daemon retirement authority is unavailable");
            }
            using var localClient = new RemoteDaemonRetirementClient(localPlan.Options);
            return await localClient.ReconcileAsync(intent, cancellationToken);
        }
        var connection = catalogStore.Load().Connections.SingleOrDefault(
            item => item.DaemonId == authorityId)
            ?? throw new InvalidDataException(
                "the daemon retirement authority changed; reopen the Hub before continuing");
        var plan = ResolveRemoteConnectionPlan(
            catalogStore,
            certificateStore,
            connection);
        using var client = new RemoteDaemonRetirementClient(plan.Options);
        return await client.ReconcileAsync(intent, cancellationToken);
    }

    private static void ShowGewyvernProvisioning(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore)
    {
        try
        {
            var catalog = DesktopProductStartup.PrepareSavedCatalog(
                catalogStore.Load(),
                catalogStore,
                certificateStore);
            var authorities = catalog.Connections
                .Select(connection => new BootstrapAuthorityOption(
                    connection.DaemonId,
                    connection.DisplayName,
                    connection.Profile.Endpoint,
                    false))
                .ToList();
            if (localOrchestraService is { GewyvernProvisioningEnabled: true })
            {
                authorities.Insert(0, new BootstrapAuthorityOption(
                    "local-orchestra",
                    "Local Orchestra",
                    "Managed on this device",
                    false));
            }
            if (authorities.Count == 0)
            {
                throw new InvalidDataException(
                    "Add and authenticate a daemon authority before provisioning gewyvern.");
            }
            var operations = new ProvisioningHubOperations(
                async (authorityId, intent, cancellationToken) =>
                {
                    var state = await ExecuteProvisioningAsync(
                        catalogStore,
                        certificateStore,
                        authorityId,
                        intent,
                        cancellationToken);
                    if (state.RuntimeRegistered)
                    {
                        RefreshHub(desktop);
                    }
                    return state;
                });
            var window = new GewyvernProvisioningWindow(authorities, operations);
            if (desktop.MainWindow is { } owner)
            {
                window.Show(owner);
            }
            else
            {
                window.Show();
            }
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var description = StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            new StartupErrorWindow(description).Show();
        }
    }

    private static async Task<RemoteProvisioningSnapshot> ExecuteProvisioningAsync(
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        string authorityId,
        RemoteProvisioningIntent intent,
        CancellationToken cancellationToken)
    {
        if (authorityId == "local-orchestra")
        {
            if (localOrchestraService is not
                { GewyvernProvisioningEnabled: true } localService)
            {
                throw new InvalidDataException(
                    "local gewyvern provisioning authority is unavailable");
            }
            if (!localService.TryEnsureReady(
                    certificateStore,
                    out var localPlan,
                    out var startupError)
                || localPlan is null)
            {
                throw new InvalidDataException(
                    startupError ?? "local gewyvern provisioning authority is unavailable");
            }
            using var localClient = new RemoteProvisioningClient(localPlan.Options);
            return await localClient.ReconcileAsync(intent, cancellationToken);
        }
        var connection = catalogStore.Load().Connections.SingleOrDefault(
            item => item.DaemonId == authorityId)
            ?? throw new InvalidDataException(
                "the provisioning authority changed; reopen the Hub before continuing");
        var plan = ResolveRemoteConnectionPlan(
            catalogStore,
            certificateStore,
            connection);
        using var client = new RemoteProvisioningClient(plan.Options);
        return await client.ReconcileAsync(intent, cancellationToken);
    }

    private static void ShowGewyvernRetirement(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore)
    {
        try
        {
            var catalog = DesktopProductStartup.PrepareSavedCatalog(
                catalogStore.Load(),
                catalogStore,
                certificateStore);
            var authorities = catalog.Connections
                .Select(connection => new BootstrapAuthorityOption(
                    connection.DaemonId,
                    connection.DisplayName,
                    connection.Profile.Endpoint,
                    false))
                .ToList();
            if (localOrchestraService is { GewyvernProvisioningEnabled: true })
            {
                authorities.Insert(0, new BootstrapAuthorityOption(
                    "local-orchestra",
                    "Local Orchestra",
                    "Managed on this device",
                    false));
            }
            if (authorities.Count == 0)
            {
                throw new InvalidDataException(
                    "Add and authenticate the daemon authority that owns the runtime before retiring gewyvern.");
            }
            var operations = new RetirementHubOperations(
                async (authorityId, intent, cancellationToken) =>
                {
                    var state = await ExecuteRetirementAsync(
                        catalogStore,
                        certificateStore,
                        authorityId,
                        intent,
                        cancellationToken);
                    if (!state.RuntimeRegistered)
                    {
                        RefreshHub(desktop);
                    }
                    return state;
                });
            var window = new GewyvernRetirementWindow(authorities, operations);
            if (desktop.MainWindow is { } owner)
            {
                window.Show(owner);
            }
            else
            {
                window.Show();
            }
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var description = StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            new StartupErrorWindow(description).Show();
        }
    }

    private static async Task<RemoteRetirementSnapshot> ExecuteRetirementAsync(
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        string authorityId,
        RemoteRetirementIntent intent,
        CancellationToken cancellationToken)
    {
        if (authorityId == "local-orchestra")
        {
            if (localOrchestraService is not
                { GewyvernProvisioningEnabled: true } localService)
            {
                throw new InvalidDataException(
                    "local gewyvern retirement authority is unavailable");
            }
            if (!localService.TryEnsureReady(
                    certificateStore,
                    out var localPlan,
                    out var startupError)
                || localPlan is null)
            {
                throw new InvalidDataException(
                    startupError ?? "local gewyvern retirement authority is unavailable");
            }
            using var localClient = new RemoteRetirementClient(localPlan.Options);
            return await localClient.ReconcileAsync(intent, cancellationToken);
        }
        var connection = catalogStore.Load().Connections.SingleOrDefault(
            item => item.DaemonId == authorityId)
            ?? throw new InvalidDataException(
                "the retirement authority changed; reopen the Hub before continuing");
        var plan = ResolveRemoteConnectionPlan(
            catalogStore,
            certificateStore,
            connection);
        using var client = new RemoteRetirementClient(plan.Options);
        return await client.ReconcileAsync(intent, cancellationToken);
    }

    private static async Task PromoteBoundBootstrapAsync(
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore,
        string authorityId,
        RemoteBootstrapSnapshot state,
        CancellationToken cancellationToken)
    {
        if (authorityId != "local-orchestra"
            || localOrchestraService is not { BootstrapEnabled: true }
            || localOrchestraService.BootstrapTrustRoot is not { } trustRoot
            || state is not
            {
                Phase: "session_bound",
                MutationAuthorized: true,
                Endpoint: not null,
                SessionCredentialHandle: not null,
                TrustCredentialHandle: not null,
            })
        {
            throw new InvalidDataException(
                "connection promotion requires a locally bound bootstrap authority");
        }

        var promotion = new DesktopBootstrapPromotion(
            catalogStore,
            certificateStore,
            trustRoot,
            PlatformRemoteTokenStore.Instance,
            BootstrapSessionCredentialResolver.Resolve,
            async (options, token) =>
            {
                using var health = new RemoteHealthClient(options);
                _ = await health.CheckAsync(token);
            });
        _ = await promotion.PromoteAsync(state, cancellationToken);
    }

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
                    var requestedProfile = RequestedProfile(
                        request,
                        certificateStore,
                        manageCertificate: true);
                    var plan = request.BootstrapTrustHandle is null
                        ? DesktopProductStartup.Resolve(requestedProfile, request.Token)
                        : DesktopProductStartup.Resolve(
                            requestedProfile,
                            certificateStore,
                            request.Token);
                    if (request.Remember)
                    {
                        var savedConnection = DesktopDaemonConnection.FromProfile(
                            requestedProfile);
                        catalogStore.Upsert(savedConnection, connection?.DaemonId);
                    }
                    certificateStore.PruneExcept(RetainedCertificatePaths(
                        catalogStore.Load(),
                        certificateStore));
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

    private static RemoteMainWindow OpenProductRemoteWindow(
        IClassicDesktopStyleApplicationLifetime desktop,
        DesktopProductStartupPlan plan)
    {
        var sessionKey = RemoteClientOptions.ParseEndpoint(plan.Profile.Endpoint).ToString();
        if (daemonSessions.TryGetValue(sessionKey, out var existing))
        {
            existing.Show();
            existing.Activate();
            return existing;
        }
        var remote = CreateProductRemoteWindow(desktop, plan);
        RegisterMainWindowLifecycle(desktop, remote);
        daemonSessions.Add(sessionKey, remote);
        remote.Closed += (_, _) => daemonSessions.Remove(sessionKey);
        remote.Show();
        return remote;
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
            var certificateStore = DesktopCertificateAuthorityStore.Default();
            var profile = RequestedProfile(request, certificateStore, manageCertificate: false);
            var certificateAuthorityPath = profile.CertificateAuthorityPath
                ?? DesktopProductStartup.ResolveCertificateAuthorityPath(
                    profile,
                    certificateStore);
            var options = RemoteClientOptions.Create(
                request.Endpoint,
                certificateAuthorityPath,
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

    private static DesktopConnectionProfile RequestedProfile(
        DesktopConnectionRequest request,
        DesktopCertificateAuthorityStore certificateStore,
        bool manageCertificate)
    {
        if (request.BootstrapTrustHandle is not null)
        {
            return new DesktopConnectionProfile
            {
                SchemaVersion = 1,
                Endpoint = request.Endpoint,
                BootstrapTrustRoot = request.BootstrapTrustRoot,
                BootstrapTrustHandle = request.BootstrapTrustHandle,
            };
        }
        return new DesktopConnectionProfile
        {
            SchemaVersion = 1,
            Endpoint = request.Endpoint,
            CertificateAuthorityPath = manageCertificate && request.Remember
                ? certificateStore.Import(request.CertificateAuthorityPath)
                : Path.GetFullPath(request.CertificateAuthorityPath),
        };
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
            certificateStore.PruneExcept(RetainedCertificatePaths(catalog, certificateStore));
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
        DesktopConnectionCatalog catalog,
        DesktopCertificateAuthorityStore certificateStore)
    {
        foreach (var connection in catalog.Connections)
        {
            yield return DesktopProductStartup.ResolveCertificateAuthorityPath(
                connection.Profile,
                certificateStore);
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
