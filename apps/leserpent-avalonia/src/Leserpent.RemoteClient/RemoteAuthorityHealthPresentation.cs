public sealed record RemoteAuthorityHealthPresentation(
    string Label,
    string AutomationName,
    bool IsSaturated)
{
    public static RemoteAuthorityHealthPresentation Create(RemoteHealth health)
    {
        if (health.Status != "ready"
            || !health.AuthorityOwned
            || health.ProtocolSchemaVersion != 1)
        {
            throw new InvalidDataException(
                "authority health presentation requires a ready protocol-v1 authority");
        }
        if (health.EffectQueue is not { } queue)
        {
            return new RemoteAuthorityHealthPresentation(
                "AUTHORITY / ready",
                "Remote authority ready; effect queue metrics unavailable",
                false);
        }
        var label = queue.Saturated
            ? $"QUEUE SATURATED / {queue.Active}/{queue.Capacity}"
            : $"QUEUE / {queue.Active}/{queue.Capacity}";
        return new RemoteAuthorityHealthPresentation(
            label,
            $"Remote authority ready; effect queue active {queue.Active} of {queue.Capacity}; saturated {queue.Saturated.ToString().ToLowerInvariant()}",
            queue.Saturated);
    }

    public static void VerifyContract()
    {
        var ready = Create(new RemoteHealth("ready", true, 1, null));
        var nominal = Create(new RemoteHealth(
            "ready",
            true,
            1,
            new RemoteEffectQueueHealth(2, 1, 4, 0, 3, 4, 16, false)));
        var saturated = Create(new RemoteHealth(
            "ready",
            true,
            1,
            new RemoteEffectQueueHealth(16, 0, 4, 0, 16, 4, 16, true)));
        if (ready.Label != "AUTHORITY / ready"
            || ready.IsSaturated
            || nominal.Label != "QUEUE / 3/16"
            || nominal.IsSaturated
            || saturated.Label != "QUEUE SATURATED / 16/16"
            || !saturated.IsSaturated
            || !saturated.AutomationName.Contains("saturated true", StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "authority health presentation contract drifted");
        }
    }
}
