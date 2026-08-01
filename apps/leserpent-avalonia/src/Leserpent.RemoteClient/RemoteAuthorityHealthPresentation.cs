public sealed record RemoteAuthorityHealthPresentation(
    string Label,
    string AutomationName,
    bool IsSaturated,
    bool RequiresAttention)
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
        var queue = health.EffectQueue;
        var replay = health.OrchestraDeleteReplayHorizon;
        if (replay is not null
            && replay.AdmissionPressure
                != RemoteOrchestraDeleteReplayAdmissionPressure.Healthy)
        {
            var pressure = replay.AdmissionPressure switch
            {
                RemoteOrchestraDeleteReplayAdmissionPressure.Warning => "WARNING",
                RemoteOrchestraDeleteReplayAdmissionPressure.Critical => "CRITICAL",
                RemoteOrchestraDeleteReplayAdmissionPressure.Blocked => "BLOCKED",
                _ => throw new InvalidDataException(
                    "attention-worthy replay pressure is invalid"),
            };
            var queueDetail = queue is null
                ? "effect queue metrics unavailable"
                : $"effect queue active {queue.Active} of {queue.Capacity}; effect queue saturated {queue.Saturated.ToString().ToLowerInvariant()}";
            return new RemoteAuthorityHealthPresentation(
                replay.AdmissionPressure
                    == RemoteOrchestraDeleteReplayAdmissionPressure.Blocked
                    ? $"REPLAY BLOCKED / {replay.Retained}/{replay.Capacity}"
                    : $"REPLAY {pressure} / {replay.AvailableCapacity} free",
                $"Remote authority ready; Orchestra delete replay pressure {pressure.ToLowerInvariant()}; available capacity {replay.AvailableCapacity} of {replay.Capacity}; checkpoint lag {replay.CheckpointLagGenerations} generations; operator action persist audit and advance checkpoint; {queueDetail}",
                replay.Saturated || queue?.Saturated == true,
                true);
        }
        if (queue is null)
        {
            var replayDetail = replay is null
                ? "Orchestra delete replay metrics unavailable"
                : $"Orchestra delete replay pressure healthy; available capacity {replay.AvailableCapacity} of {replay.Capacity}";
            return new RemoteAuthorityHealthPresentation(
                "AUTHORITY / ready",
                $"Remote authority ready; effect queue metrics unavailable; {replayDetail}",
                replay?.Saturated == true,
                false);
        }
        var label = queue.Saturated
            ? $"QUEUE SATURATED / {queue.Active}/{queue.Capacity}"
            : $"QUEUE / {queue.Active}/{queue.Capacity}";
        return new RemoteAuthorityHealthPresentation(
            label,
            $"Remote authority ready; effect queue active {queue.Active} of {queue.Capacity}; saturated {queue.Saturated.ToString().ToLowerInvariant()}",
            queue.Saturated,
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
        var replayCritical = Create(new RemoteHealth(
            "ready",
            true,
            1,
            null,
            OrchestraDeleteReplayHorizon: new RemoteOrchestraDeleteReplayHorizon(
                4_096,
                4_000,
                96,
                512,
                128,
                768,
                256,
                3_999,
                false,
                RemoteOrchestraDeleteReplayAdmissionState.Ready,
                RemoteOrchestraDeleteReplayAdmissionPressure.Critical,
                RemoteOrchestraDeleteReplayOperatorAction
                    .PersistAuditAndAdvanceCheckpoint,
                1,
                4_000,
                4_001,
                0,
                1,
                1)));
        if (ready.Label != "AUTHORITY / ready"
            || ready.IsSaturated
            || ready.RequiresAttention
            || nominal.Label != "QUEUE / 3/16"
            || nominal.IsSaturated
            || nominal.RequiresAttention
            || saturated.Label != "QUEUE SATURATED / 16/16"
            || !saturated.IsSaturated
            || !saturated.RequiresAttention
            || !saturated.AutomationName.Contains("saturated true", StringComparison.Ordinal)
            || replayCritical.Label != "REPLAY CRITICAL / 96 free"
            || replayCritical.IsSaturated
            || !replayCritical.RequiresAttention
            || !replayCritical.AutomationName.Contains(
                "operator action persist audit and advance checkpoint",
                StringComparison.Ordinal)
            || !replayCritical.AutomationName.Contains(
                "checkpoint lag 3999 generations",
                StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "authority health presentation contract drifted");
        }
    }
}
