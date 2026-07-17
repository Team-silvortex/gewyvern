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
