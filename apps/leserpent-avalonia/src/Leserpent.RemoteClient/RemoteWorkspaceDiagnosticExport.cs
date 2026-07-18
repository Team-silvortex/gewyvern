using System.Text;

public static class RemoteWorkspaceDiagnosticExport
{
    public const int MaxUtf8Bytes = 512 * 1024;

    public static string Create(RemoteWorkspaceLogView view)
    {
        Validate(view);
        var snapshot = view.Snapshot;
        var output = new StringBuilder();
        output.AppendLine("leserpent.workspace-diagnostic/v1");
        Append(output, "runtime.id", snapshot.Runtime.Id);
        Append(output, "runtime.name", snapshot.Runtime.Name);
        output.Append("revision = ").AppendLine(snapshot.Revision.ToString());
        output.Append("history.count = ").AppendLine(snapshot.History.Count.ToString());
        output.AppendLine("history:");
        foreach (var entry in snapshot.History)
        {
            output.Append("  - revision = ").Append(entry.Revision)
                .Append("; command_id = ").Append(Quote(entry.CommandId))
                .Append("; status = ").AppendLine(Quote(entry.Status));
        }
        output.Append("logs.visible = ").AppendLine(view.VisibleLogCount.ToString());
        output.Append("logs.total = ").AppendLine(view.TotalLogCount.ToString());
        Append(output, "logs.filter.query", view.Query);
        Append(output, "logs.filter.level", view.Level);
        output.AppendLine("logs:");
        foreach (var entry in snapshot.Logs)
        {
            output.Append("  - sequence = ").Append(entry.Sequence)
                .Append("; level = ").Append(Quote(entry.Level))
                .Append("; display = ").AppendLine(Quote(entry.Display));
        }
        var result = output.ToString();
        if (Encoding.UTF8.GetByteCount(result) > MaxUtf8Bytes)
        {
            throw new InvalidDataException("workspace diagnostic export exceeds its size limit");
        }
        return result;
    }

    public static byte[] Encode(RemoteWorkspaceLogView view)
    {
        var encoded = Encoding.UTF8.GetBytes(Create(view));
        if (encoded.Length > MaxUtf8Bytes)
        {
            throw new InvalidDataException("workspace diagnostic export exceeds its size limit");
        }
        return encoded;
    }

    public static string SuggestedFileName(RemoteWorkspaceSnapshot snapshot)
    {
        var runtime = new string(snapshot.Runtime.Id
            .Where(character => char.IsAsciiLetterOrDigit(character)
                || character is '-' or '_')
            .Take(48)
            .ToArray());
        if (runtime.Length == 0)
        {
            runtime = "runtime";
        }
        return $"leserpent-{runtime}-r{snapshot.Revision}.txt";
    }

    public static void VerifyContract()
    {
        var snapshot = new RemoteWorkspaceSnapshot(
            9,
            new RemoteRuntimeProjection
            {
                Id = "runtime-a",
                Name = "Payments\nAPI",
                Revision = 9,
                Tags = new RuntimeTags(),
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            },
            [new RemoteHistoryProjection("command-a", 8, "applied")],
            [
                new RemoteLogProjection(1, "info", "ready"),
                new RemoteLogProjection(2, "warning", "retry \"soon\""),
            ]);
        var view = RemoteWorkspaceLogFilter.Apply(snapshot, "retry", "warning");
        var exported = Create(view);
        var expected = """
            leserpent.workspace-diagnostic/v1
            runtime.id = "runtime-a"
            runtime.name = "Payments\nAPI"
            revision = 9
            history.count = 1
            history:
              - revision = 8; command_id = "command-a"; status = "applied"
            logs.visible = 1
            logs.total = 2
            logs.filter.query = "retry"
            logs.filter.level = "warning"
            logs:
              - sequence = 2; level = "warning"; display = "retry \"soon\""

            """;
        if (!string.Equals(exported, expected, StringComparison.Ordinal)
            || exported.Contains("ready", StringComparison.Ordinal)
            || Encoding.UTF8.GetByteCount(exported) > MaxUtf8Bytes)
        {
            throw new InvalidDataException("workspace diagnostic export contract drifted");
        }
        var encoded = Encode(view);
        var unsafeNameSnapshot = snapshot with
        {
            Runtime = new RemoteRuntimeProjection
            {
                Id = "../../\n",
                Name = snapshot.Runtime.Name,
                Revision = snapshot.Runtime.Revision,
                Tags = new RuntimeTags(),
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            },
        };
        if (encoded.Length != Encoding.UTF8.GetByteCount(exported)
            || encoded.AsSpan().StartsWith(Encoding.UTF8.Preamble)
            || SuggestedFileName(snapshot) != "leserpent-runtime-a-r9.txt"
            || SuggestedFileName(unsafeNameSnapshot) != "leserpent-runtime-r9.txt")
        {
            throw new InvalidDataException("workspace diagnostic file contract drifted");
        }

        var maximalLogs = Enumerable.Range(1, RemoteWorkspaceClient.MaxLogEntries)
            .Select(index => new RemoteLogProjection(
                (ulong)index,
                "error",
                new string('\\', RemoteWorkspaceClient.MaxLogDisplayBytes)))
            .ToArray();
        var maximal = Create(RemoteWorkspaceLogFilter.Apply(
            snapshot with { Logs = maximalLogs },
            null,
            RemoteWorkspaceLogFilter.AllLevels));
        var maximalBytes = Encoding.UTF8.GetByteCount(maximal);
        if (maximalBytes <= 256 * 1024 || maximalBytes > MaxUtf8Bytes)
        {
            throw new InvalidDataException(
                "workspace diagnostic export does not cover a maximally escaped snapshot");
        }
    }

    private static void Validate(RemoteWorkspaceLogView view)
    {
        if (view.VisibleLogCount != view.Snapshot.Logs.Count
            || view.VisibleLogCount > view.TotalLogCount
            || view.Query.Length > RemoteWorkspaceLogFilter.MaxQueryLength
            || !RemoteWorkspaceLogFilter.Levels.Contains(
                view.Level,
                StringComparer.Ordinal))
        {
            throw new InvalidDataException("workspace diagnostic view is inconsistent");
        }
    }

    private static void Append(StringBuilder output, string key, string value) =>
        output.Append(key).Append(" = ").AppendLine(Quote(value));

    private static string Quote(string value)
    {
        var output = new StringBuilder(value.Length + 2).Append('"');
        foreach (var character in value)
        {
            _ = character switch
            {
                '\\' => output.Append("\\\\"),
                '"' => output.Append("\\\""),
                '\n' => output.Append("\\n"),
                '\r' => output.Append("\\r"),
                '\t' => output.Append("\\t"),
                _ when char.IsControl(character) => output.Append("\\u")
                    .Append(((int)character).ToString("x4")),
                _ => output.Append(character),
            };
        }
        return output.Append('"').ToString();
    }
}
