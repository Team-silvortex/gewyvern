namespace Leserpent.ControlPlane;

internal static class RuntimeCapabilityProjection
{
    public static IReadOnlyList<RuntimeCapability> ToLegacy(
        RuntimeCapabilityAuthoritySnapshot snapshot)
    {
        var capabilities = new List<RuntimeCapability>
        {
            new("api.latest_snapshot", snapshot.LatestSnapshot ? "fully_supported" : "not_supported", "runtime publishes latest snapshot metadata and JSON surfaces"),
            new("control.authenticated_deployment", snapshot.AuthenticatedDeployment ? "fully_supported" : "not_supported", "runtime accepts typed, token-authenticated deployment requests"),
            new("api.target_routing", "fully_supported", $"target routing uses {snapshot.TargetPathSegmentEncoding} path encoding"),
            new("api.external_sidecar_context", snapshot.ExternalSidecarContext ? "fully_supported" : "not_supported", "runtime can expose additive nearby sidecar collaboration context"),
            new("runtime.serve_required", snapshot.ServeRequired ? "fully_supported" : "not_supported", "runtime requires standalone serve mode for latest-snapshot API access"),
        };

        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/meta", "api.latest.meta", "latest snapshot metadata surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/targets", "api.latest.targets", "latest target index surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/summary.json", "api.summary_json", "machine-facing summary JSON surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/analysis.json", "api.analysis_json", "machine-facing analysis JSON surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/training-example.json", "api.training_example_json", "machine-facing training example JSON surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/training-dataset.json", "api.training_dataset_manifest", "training dataset manifest surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/export.json", "api.export_json", "machine-facing export JSON surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/report.json", "api.report_json", "machine-facing report JSON surface");
        AddEndpoint(capabilities, snapshot.Endpoints, "/v1/latest/report.html", "api.report_html", "human-facing HTML report surface");
        return capabilities
            .OrderBy(capability => capability.Key, StringComparer.OrdinalIgnoreCase)
            .ToArray();
    }

    private static void AddEndpoint(
        List<RuntimeCapability> capabilities,
        IReadOnlyList<string> endpoints,
        string path,
        string key,
        string description) =>
        capabilities.Add(new RuntimeCapability(
            key,
            endpoints.Contains(path, StringComparer.OrdinalIgnoreCase)
                ? "fully_supported"
                : "not_supported",
            description));
}
