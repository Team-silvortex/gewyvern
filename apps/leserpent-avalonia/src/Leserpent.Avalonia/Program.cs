using Avalonia;

internal static class Program
{
    [STAThread]
    public static int Main(string[] args)
    {
        if (args is ["--verify-remote-filter"])
        {
            RemoteDocumentProjection.VerifyFilterContract();
            Console.WriteLine(
                "remote filter valid: renderer_neutral=true, topology=true, authority=true, name=true, id=true, tag=true, status=true, bounded=true, empty_state=true");
            return 0;
        }
        if (args is ["--verify-remote-topology"])
        {
            RemoteTopologyCodec.VerifyContract();
            Console.WriteLine(
                "remote topology valid: typed_runtime_list=true, bounded=true, strict_decode=true, null_runtime_rejected=true, typed_query_error=true, unique_runtime_ids=true, revision_fenced=true, runtime_endpoint_retained=false");
            return 0;
        }
        if (args is ["--verify-credential-source"])
        {
            RemoteCredentialPresentation.VerifyContract();
            Console.WriteLine(
                "credential source presentation valid: platform=true, environment_fallback=true, token_value_input=false");
            return 0;
        }
        if (args is ["--verify-remote-layout"])
        {
            RemoteResponsiveLayout.VerifyContract();
            Console.WriteLine(
                "remote responsive layout valid: min_width=compact, breakpoint=780, default_width=wide");
            return 0;
        }
        if (args is ["--verify-authority-health-presentation"])
        {
            RemoteAuthorityHealthPresentation.VerifyContract();
            RemoteAuthorityHealthCoordinator.VerifyContractAsync()
                .GetAwaiter()
                .GetResult();
            Console.WriteLine(
                "authority health presentation valid: ready=true, queue_pressure=true, saturation_visible=true, shared_lifecycle=true, single_flight=true, stop_fence=true, endpoint_retained=false");
            return 0;
        }
        if (args is ["--verify-leselang-gui-export"])
        {
            RemoteLeselangClient.VerifyContract();
            Console.WriteLine(
                "GUI Leselang export protocol valid: rust_authority=true, authenticated_route=true, strict_decode=true, execution=false");
            return 0;
        }
        if (args is ["--verify-remote-mutation-fence"])
        {
            RemoteMutationFences.VerifyContract();
            RemoteMutationCoordinator.VerifyContract();
            Console.WriteLine(
                "remote mutation fence valid: command_revision=true, capability_observation_revision=true, heartbeat_blocked=true, authoritative_snapshot=true, cached_heartbeat_admission=false, pending_projection_blocked=true, malformed_response_unknown=true, shared_coordinator=true, shared_failure_classification=true, stale_failure_ignored=true, bounded_failure_diagnostics=true, action_availability=true");
            return 0;
        }
        if (args is ["--verify-remote-event-lifecycle"])
        {
            RemoteEventClient.VerifyLifecycleContractAsync()
                .GetAwaiter()
                .GetResult();
            Console.WriteLine(
                "remote event lifecycle valid: dispose_single_flight=true, resource_release_once=true, restart_identity=true, stopped_start_rejected=true, subscriber_failure_isolated=true, subscriber_failure_count_bounded=true");
            return 0;
        }
        if (args is ["--verify-remote-ui-action-routing"])
        {
            RemoteUiActionRouter.VerifyContract();
            Console.WriteLine(
                "remote UI action routing valid: typed_binding=true, opaque_node_ids=true, runtime_context=true, availability=true, deployment_form=true, submission_source_fence=true, bounded_failure=true");
            return 0;
        }
        if (args is ["--verify-deployment-contract"])
        {
            RemoteMutationClient.VerifyDeploymentContract();
            Console.WriteLine(
                "deployment mutation contract valid: typed=true, confirmed=true, bounded=true, null_omission=true");
            return 0;
        }
        if (args is ["--verify-bootstrap-client"])
        {
            RemoteBootstrapClient.VerifyContract();
            BootstrapSessionCredentialResolver.VerifyContract();
            Console.WriteLine(
                "bootstrap client valid: submit_route=true, inspect_wire=true, bind_wire=true, strict_state=true, bounded=true, opaque_handles=true, retirement_authority=true, rust_secret_schema=true, raw_secrets=false");
            return 0;
        }
        if (args is ["--verify-provisioning-client"])
        {
            RemoteProvisioningClient.VerifyContract();
            Console.WriteLine(
                "provisioning client valid: https_route=true, strict_state=true, bounded=true, stable_identity=true, opaque_handles=true, raw_secrets=false, runtime_deploy_independent=true");
            return 0;
        }
        if (args is ["--verify-retirement-client"])
        {
            RemoteRetirementClient.VerifyContract();
            Console.WriteLine(
                "retirement client valid: https_route=true, strict_state=true, bounded=true, stable_identity=true, provisioning_bound=true, opaque_handles=true, raw_secrets=false, failure_preserves_registration=true");
            return 0;
        }
        if (args is ["--verify-daemon-retirement-client"])
        {
            RemoteDaemonRetirementClient.VerifyContract();
            Console.WriteLine(
                "daemon retirement client valid: https_route=true, strict_state=true, bounded=true, stable_identity=true, bootstrap_bound=true, authority_omitting=true, opaque_handles=true, raw_secrets=false, runtime_retirement_independent=true");
            return 0;
        }
        if (args is ["--verify-bootstrap-promotion"])
        {
            DesktopBootstrapPromotion.VerifyContract();
            Console.WriteLine(
                "bootstrap promotion valid: session_bound_only=true, endpoint_trust_bound=true, rust_secret_handle=true, health_before_persist=true, platform_vault=true, catalog_secret_free=true, conflicting_credential_rejected=true");
            return 0;
        }
        if (args is ["--verify-parameterized-form"])
        {
            RemoteWorkspaceDocumentProjection.VerifyParameterizedFormContract();
            Console.WriteLine(
                "parameterized form event valid: renderer_neutral=true, bounded=true, typed_submit=true, unknown_fields=false");
            return 0;
        }
        if (args is ["--verify-desktop-profile"])
        {
            DesktopConnectionProfileStore.VerifyContract();
            DesktopConnectionCatalogStore.VerifyContract();
            Console.WriteLine(
                "desktop connection catalog valid: multi_daemon=true, legacy_migration=true, stable_authority_id=true, path_trust=true, bootstrap_handle_trust=true, exclusive_trust_source=true, bounded=true, atomic=true, private=true, token_persisted=false, unknown_fields=false");
            return 0;
        }
        if (args is ["--verify-desktop-ca-store"])
        {
            BootstrapTrustRecordStore.VerifyContract();
            DesktopCertificateAuthorityStore.VerifyContract();
            Console.WriteLine(
                "desktop CA store valid: single_pem=true, certificate_authority=true, content_addressed=true, atomic=true, private=true, idempotent=true, profile_migration=true, bootstrap_handle=true, endpoint_bound=true, digest_bound=true, bounded_prune=true, stale_temp_cleanup=true, unknown_fields=false, trailing_material=false, managed_replacement=false, symlink=false");
            return 0;
        }
        if (args is ["--verify-desktop-lifecycle"])
        {
            DesktopApplicationLifecycle.VerifyContract();
            Console.WriteLine(
                "desktop lifecycle valid: app_menu=true, connection_settings=true, language_settings=true, about=true, learning_center=true, offline_tutorial=true, dock_reopen=true, auxiliary_windows_not_main_window=true, explicit_quit=true");
            return 0;
        }
        if (args is ["--verify-desktop-localization"])
        {
            DesktopLocalization.VerifyContract();
            DesktopLanguagePreferenceStore.VerifyContract();
            Console.WriteLine(
                "desktop localization valid: schema=v1, official_locales=30, builtin_locales=8, builtin_shell_catalogs=8, builtin_semantic_catalogs=7, semantic_keys=26, builtin_connection_catalogs=7, connection_semantic_keys=33, builtin_bootstrap_catalogs=7, bootstrap_semantic_keys=46, builtin_provisioning_catalogs=7, provisioning_semantic_keys=43, builtin_retirement_catalogs=7, retirement_semantic_keys=45, builtin_semantic_keys=193, downloadable_locales=22, system_resolution=true, bcp47_aliases=true, persistent_preference=true, atomic=true, private=true, unknown_fields=false, localized_ui_ir=true, localized_connection_dialog=true, localized_reverse_deployment=true, localized_gewyvern_provisioning=true, localized_gewyvern_retirement=true, english_fallback=true, zh_cn_core=true, zh_cn_tutorial_complete=true, rtl_locales=3");
            return 0;
        }
        if (args is ["--verify-local-orchestra", var daemonPath])
        {
            LocalOrchestraServiceSupervisor.VerifyContract(daemonPath);
            Console.WriteLine(
                "local orchestra valid: rust_daemon=true, loopback_tls=true, ephemeral_token=true, owned_authority=true, runtime_topology_query=true, health_topology_composition=true, authority_bound_live_state=true, private_files=true, minimal_child_environment=true, optional_bootstrap_origin=true, optional_gewyvern_provisioning_origin=true, private_bootstrap_trust=true, package_local_daemon=true, symlink_rejection=true, process_cleanup=true");
            return 0;
        }
        if (args is ["--verify-connection-maintenance"])
        {
            DesktopConnectionMaintenance.VerifyContract();
            Console.WriteLine(
                "desktop connection maintenance valid: endpoint_scoped_delete=true, profile_cleared=true, stale_profile_blocked=true, environment_untouched=true");
            return 0;
        }
        if (args is ["--verify-packaged-profile-startup", var profilePath])
        {
            DesktopProductStartup.VerifyPackagedProfile(profilePath);
            Console.WriteLine(
                "packaged profile startup valid: saved_profile=true, platform_keychain=true, token_output=false, credential_cleaned=true");
            return 0;
        }
        if (args is ["--verify-remote-workspace"])
        {
            RemoteWorkspaceDocumentProjection.VerifyEndpointIsolation();
            Console.WriteLine(
                "remote workspace projection valid: semantic=true, endpoint_retained=false, history_bounded=true, logs_bounded=true");
            return 0;
        }
        if (args is ["--verify-workspace-diagnostics"]
            or ["--verify-workspace-log-filter"])
        {
            RemoteWorkspaceLogFilter.VerifyContract();
            RemoteWorkspaceDiagnosticExport.VerifyContract();
            RemoteWorkspaceLiveRefresh.VerifyContract();
            RemoteWorkspaceSnapshotChanges.VerifyContract();
            RemoteWorkspaceSeverityAlert.VerifyContract();
            RemoteWorkspaceCodec.VerifyIncrementalContract();
            RemoteWorkspaceLogRefreshPlan.VerifyContract();
            Console.WriteLine(
                "workspace diagnostics valid: local_only=true, query=true, level=true, combined=true, bounded=true, empty_state=true, command_identity=true, explicit_export=true, file_export=true, maximal_escape=true, live_refresh=true, bounded_retry=true, manual_recovery=true, skip_neutral=true, delta_summary=true, severity_signal=true, snapshot_fence=true, severity_ack=true, incremental_logs=true");
            return 0;
        }
        if (args is ["--verify-silvortex-account"])
        {
            SilvortexAccountSession.VerifyContract();
            Console.WriteLine(
                "Silvortex desktop account valid: reviewed_application=leserpent, reviewed_profile=leserpent_desktop, default_client_id=true, native_client=true, system_browser=true, pkce_s256=true, state=true, nonce=true, response_issuer=true, strict_loopback_http=true, rs256_jwks=true, mfa=true, rotating_vault=true, duplicate_parameters=false, client_secret=false, offline_mode=true, packaged_issuer=true, environment_override=false");
            return 0;
        }
        if (args is ["--verify-silvortex-account-proof"])
        {
            SilvortexAccountProof.VerifyContract();
            Console.WriteLine(
                "Silvortex desktop proof valid: reviewed_client=true, native_aot_required=true, packaged_macos_config=true, system_browser=true, fresh_session_restore=true, refresh_rotation=true, local_logout=true, private_atomic_evidence=true, identity_retained=false, credential_retained=false");
            return 0;
        }
        if (args is ["--prove-silvortex-account", var evidencePath])
        {
            try
            {
                var proof = SilvortexAccountProof.RunAsync(evidencePath)
                    .GetAwaiter()
                    .GetResult();
                Console.WriteLine(
                    $"Silvortex desktop proof passed: evidence={proof.EvidencePath}, duration_ms={proof.DurationMilliseconds}");
                return 0;
            }
            catch (Exception error) when (StartupFailure.IsExpected(error))
            {
                Console.Error.WriteLine(
                    $"Silvortex desktop proof failed: {StartupFailure.Describe(error)}");
                return StartupFailure.ExitCode;
            }
        }
        try
        {
            return BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            var description = StartupFailure.Describe(
                error,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            Console.Error.WriteLine($"Leserpent startup failed: {description}");
            return StartupFailure.ExitCode;
        }
    }

    public static AppBuilder BuildAvaloniaApp() => AppBuilder
        .Configure<LeserpentApp>()
        .UsePlatformDetect()
        .LogToTrace();
}
