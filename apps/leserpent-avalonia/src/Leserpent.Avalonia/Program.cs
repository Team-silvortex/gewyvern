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
                "remote filter valid: name=true, id=true, tag=true, status=true, bounded=true, empty_state=true");
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
        if (args is ["--verify-remote-mutation-fence"])
        {
            RemoteMainWindow.VerifyMutationFenceContract();
            Console.WriteLine(
                "remote mutation fence valid: command_revision=true, capability_observation_revision=true, heartbeat_blocked=true, authoritative_snapshot=true, pending_projection_blocked=true");
            return 0;
        }
        if (args is ["--verify-deployment-contract"])
        {
            RemoteMutationClient.VerifyDeploymentContract();
            Console.WriteLine(
                "deployment mutation contract valid: typed=true, confirmed=true, bounded=true, null_omission=true");
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
            Console.WriteLine(
                "desktop connection profile valid: bounded=true, atomic=true, private=true, token_persisted=false, unknown_fields=false");
            return 0;
        }
        if (args is ["--verify-desktop-ca-store"])
        {
            DesktopCertificateAuthorityStore.VerifyContract();
            Console.WriteLine(
                "desktop CA store valid: single_pem=true, certificate_authority=true, content_addressed=true, atomic=true, private=true, idempotent=true, profile_migration=true, bounded_prune=true, stale_temp_cleanup=true, trailing_material=false, managed_replacement=false, symlink=false");
            return 0;
        }
        if (args is ["--verify-desktop-lifecycle"])
        {
            DesktopApplicationLifecycle.VerifyContract();
            Console.WriteLine(
                "desktop lifecycle valid: app_menu=true, connection_settings=true, about=true, dock_reopen=true, explicit_quit=true");
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
