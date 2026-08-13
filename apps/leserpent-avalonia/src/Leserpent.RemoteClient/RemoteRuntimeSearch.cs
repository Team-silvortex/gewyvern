public sealed record RemoteTopologySearchItem(
    string AuthorityId,
    string AuthorityName,
    string AuthorityKind,
    string AuthorityDetail,
    IReadOnlyList<RemoteRuntimeProjection> Runtimes);

public sealed record RemoteTopologySearchResult(
    IReadOnlyDictionary<string, IReadOnlyList<RemoteRuntimeProjection>> RuntimesByAuthority,
    IReadOnlySet<string> VisibleAuthorityIds,
    int VisibleAuthorityCount,
    int TotalAuthorityCount,
    int VisibleRuntimeCount,
    int TotalRuntimeCount,
    string Filter);

public static class RemoteRuntimeSearch
{
    public const int MaxFilterLength = 128;

    public static string SanitizeInput(string? filter) => new string((filter ?? string.Empty)
        .Where(character => !char.IsControl(character))
        .Take(MaxFilterLength)
        .ToArray());

    public static string Normalize(string? filter) => SanitizeInput(filter).Trim();

    public static bool Matches(RemoteRuntimeProjection runtime, string? filter)
    {
        ArgumentNullException.ThrowIfNull(runtime);
        var normalized = Normalize(filter);
        return normalized.Length == 0 || RuntimeValues(runtime).Any(value =>
            value?.Contains(normalized, StringComparison.OrdinalIgnoreCase) == true);
    }

    public static RemoteTopologySearchResult FilterTopology(
        IEnumerable<RemoteTopologySearchItem> authorities,
        string? filter)
    {
        ArgumentNullException.ThrowIfNull(authorities);
        var source = authorities.ToArray();
        if (source.Any(authority => string.IsNullOrWhiteSpace(authority.AuthorityId))
            || source.Select(authority => authority.AuthorityId)
                .Distinct(StringComparer.Ordinal).Count() != source.Length)
        {
            throw new InvalidDataException(
                "topology search authorities require unique non-empty identities");
        }

        var normalized = Normalize(filter);
        var result = new Dictionary<string, IReadOnlyList<RemoteRuntimeProjection>>(
            StringComparer.Ordinal);
        var visibleAuthorityIds = new HashSet<string>(StringComparer.Ordinal);
        var visibleAuthorities = 0;
        var visibleRuntimes = 0;
        var totalRuntimes = 0;
        foreach (var authority in source)
        {
            totalRuntimes = checked(totalRuntimes + authority.Runtimes.Count);
            var authorityMatches = normalized.Length == 0 || AuthorityValues(authority).Any(value =>
                value.Contains(normalized, StringComparison.OrdinalIgnoreCase));
            var runtimes = authorityMatches
                ? authority.Runtimes.ToArray()
                : authority.Runtimes.Where(runtime => Matches(runtime, normalized)).ToArray();
            result.Add(authority.AuthorityId, runtimes);
            if (authorityMatches || runtimes.Length > 0)
            {
                visibleAuthorityIds.Add(authority.AuthorityId);
                visibleAuthorities++;
            }
            visibleRuntimes = checked(visibleRuntimes + runtimes.Length);
        }
        return new RemoteTopologySearchResult(
            result,
            visibleAuthorityIds,
            visibleAuthorities,
            source.Length,
            visibleRuntimes,
            totalRuntimes,
            normalized);
    }

    public static void VerifyContract()
    {
        var runtimes = new[]
        {
            Runtime("runtime-a", "Payments API", "production", RefreshStatus.Ready),
            Runtime("runtime-b", "Queue Worker", "staging", RefreshStatus.Failed),
        };
        if (!Matches(runtimes[0], "payments")
            || !Matches(runtimes[1], "RUNTIME-B")
            || !Matches(runtimes[0], "PRODUCTION")
            || !Matches(runtimes[1], "failed")
            || !Matches(runtimes[0], "prod\0uction")
            || Matches(runtimes[0], "does-not-exist")
            || Normalize(new string('x', MaxFilterLength + 32)).Length != MaxFilterLength)
        {
            throw new InvalidDataException("renderer-neutral runtime search drifted");
        }

        var topology = new[]
        {
            new RemoteTopologySearchItem(
                "local-orchestra",
                "Local Orchestra",
                "LOCAL",
                "Managed on this device",
                [runtimes[0]]),
            new RemoteTopologySearchItem(
                "remote-alpha",
                "Alpha Authority",
                "REMOTE",
                "https://alpha.example:9443",
                [runtimes[1]]),
        };
        var runtimeMatch = FilterTopology(topology, "queue");
        var authorityMatch = FilterTopology(topology, "local");
        var all = FilterTopology(topology, "  ");
        var empty = FilterTopology(topology, "does-not-exist");
        if (runtimeMatch is not { VisibleAuthorityCount: 1, VisibleRuntimeCount: 1 }
            || runtimeMatch.RuntimesByAuthority["remote-alpha"] is not [{ Id: "runtime-b" }]
            || authorityMatch is not { VisibleAuthorityCount: 1, VisibleRuntimeCount: 1 }
            || !authorityMatch.VisibleAuthorityIds.SetEquals(["local-orchestra"])
            || authorityMatch.RuntimesByAuthority["local-orchestra"] is not [{ Id: "runtime-a" }]
            || all is not
            {
                VisibleAuthorityCount: 2,
                TotalAuthorityCount: 2,
                VisibleRuntimeCount: 2,
                TotalRuntimeCount: 2,
            }
            || empty is not { VisibleAuthorityCount: 0, VisibleRuntimeCount: 0 })
        {
            throw new InvalidDataException("renderer-neutral topology search drifted");
        }

        try
        {
            _ = FilterTopology([topology[0], topology[0]], null);
            throw new InvalidDataException("topology search accepted duplicate authority IDs");
        }
        catch (InvalidDataException error) when (
            error.Message == "topology search authorities require unique non-empty identities")
        {
        }
    }

    private static IEnumerable<string?> RuntimeValues(RemoteRuntimeProjection runtime) =>
    [
        runtime.Id,
        runtime.Name,
        runtime.RefreshStatus.ToString(),
        runtime.Tags.Environment,
        runtime.Tags.Cluster,
        runtime.Tags.Role,
        runtime.Status.StatusSource,
        runtime.Status.StatusFetchError,
        runtime.Status.ResilienceStatus,
        runtime.Capabilities?.Service,
        runtime.Capabilities?.Version,
    ];

    private static IEnumerable<string> AuthorityValues(RemoteTopologySearchItem authority) =>
    [
        authority.AuthorityId,
        authority.AuthorityName,
        authority.AuthorityKind,
        authority.AuthorityDetail,
    ];

    private static RemoteRuntimeProjection Runtime(
        string id,
        string name,
        string environment,
        RefreshStatus refreshStatus) => new()
    {
        Id = id,
        Name = name,
        Revision = 9,
        RefreshStatus = refreshStatus,
        Tags = new RuntimeTags { Environment = environment },
        Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
    };
}
