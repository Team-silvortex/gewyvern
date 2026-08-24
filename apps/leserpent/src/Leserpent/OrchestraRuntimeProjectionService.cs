using Leserpent.ControlPlane;

namespace Leserpent;

public sealed record OrchestraRuntimeProjection(
    RuntimeSummary Runtime,
    RuntimeAttentionView Attention,
    IReadOnlyList<OrchestraPlan> Plans);

public sealed class OrchestraRuntimeProjectionService(
    RuntimeReadProjectionService runtimeReads,
    RegistryService registry)
{
    public async Task<OrchestraRuntimeProjection?> ReadAsync(
        string runtimeId,
        CancellationToken cancellationToken)
    {
        var runtime = await runtimeReads.InspectAsync(runtimeId, cancellationToken);
        if (runtime is null)
        {
            return null;
        }

        var attention = registry.GetRuntimeAttention(runtimeId, runtime);
        if (attention is null)
        {
            return null;
        }

        return new OrchestraRuntimeProjection(
            runtime,
            attention,
            OrchestraPlanner.Build(
                runtime,
                attention.Reasons,
                attention.Severity,
                attention.NeedsAttention));
    }
}
