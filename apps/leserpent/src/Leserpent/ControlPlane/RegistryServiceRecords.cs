namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    private sealed record RuntimeRecord(
        string RuntimeId,
        string Name,
        string Endpoint,
        string? RuntimeAdminToken,
        string? SidecarEndpoint,
        string? SidecarAdminToken,
        DateTimeOffset RegisteredAt,
        DateTimeOffset UpdatedAt,
        IReadOnlyList<RuntimeCapability> Capabilities,
        string CapabilitySource,
        DateTimeOffset? CapabilityFetchedAt,
        string? CapabilityFetchError,
        RuntimeTags Tags,
        RuntimeStatusSnapshot Status,
        RuntimeSidecarStatusSnapshot? SidecarStatus)
    {
        public RuntimeRegistrationResponse ToRegistrationResponse() =>
            new(
                RuntimeId,
                Name,
                Endpoint,
                SidecarEndpoint,
                !string.IsNullOrWhiteSpace(SidecarAdminToken),
                RegisteredAt,
                Capabilities,
                CapabilitySource,
                CapabilityFetchedAt,
                CapabilityFetchError,
                Tags,
                Status,
                SidecarStatus,
                !string.IsNullOrWhiteSpace(RuntimeAdminToken));

        public RuntimeSummary ToSummary() =>
            new(
                RuntimeId,
                Name,
                Endpoint,
                SidecarEndpoint,
                !string.IsNullOrWhiteSpace(SidecarAdminToken),
                RegisteredAt,
                UpdatedAt,
                Capabilities,
                CapabilitySource,
                CapabilityFetchedAt,
                CapabilityFetchError,
                Tags,
                Status,
                SidecarStatus,
                !string.IsNullOrWhiteSpace(RuntimeAdminToken));

        public PersistedRuntimeState ToPersistedState() =>
            new(RuntimeId, Name, Endpoint, SidecarEndpoint, RegisteredAt, UpdatedAt, Capabilities, CapabilitySource, CapabilityFetchedAt, CapabilityFetchError, Tags, Status, SidecarStatus);
    }

    private sealed record SessionRecord(
        string SessionId,
        string RuntimeId,
        string PipelineKind,
        string RequestedBy,
        string Status,
        DateTimeOffset CreatedAt,
        DateTimeOffset UpdatedAt,
        IReadOnlyList<SessionCapabilityRequirement> Requirements)
    {
        public SessionSummary ToSummary() =>
            new(SessionId, RuntimeId, PipelineKind, RequestedBy, Status, CreatedAt, UpdatedAt, Requirements);

        public PersistedSessionState ToPersistedState() =>
            new(SessionId, RuntimeId, PipelineKind, RequestedBy, Status, CreatedAt, UpdatedAt, Requirements);
    }
}
