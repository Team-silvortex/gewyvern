using System.Text;

public static class RemoteLeselangExport
{
    private const int MaxSourceLength = 4096;

    public static string Refresh(string runtimeId, bool capabilities)
    {
        RemoteQueryValidation.RequireIdentifier(runtimeId, "runtime ID");
        var operation = capabilities ? "refresh_capabilities" : "refresh";
        return Bounded(
            $"fn main() = runtime.{operation}(runtime_id: {Quote(runtimeId)})\n");
    }

    public static string Deploy(
        string runtimeId,
        string pipelineKind,
        string? target)
    {
        RemoteQueryValidation.RequireIdentifier(runtimeId, "runtime ID");
        RemoteMutationClient.RequireDeploymentToken(pipelineKind, "pipeline kind");
        if (target is not null)
        {
            RemoteMutationClient.RequireDeploymentTarget(target);
        }
        return Bounded(
            "fn main() = runtime.deploy(\n"
            + $"  runtime_id: {Quote(runtimeId)},\n"
            + $"  pipeline_kind: {Quote(pipelineKind)},\n"
            + $"  target: {(target is null ? "none" : Quote(target))},\n"
            + ")\n");
    }

    public static void VerifyContract()
    {
        if (Refresh("runtime-a", false)
                != "fn main() = runtime.refresh(runtime_id: \"runtime-a\")\n"
            || Refresh("runtime-a", true)
                != "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")\n"
            || Deploy("runtime-a", "http/request", "pid:42")
                != "fn main() = runtime.deploy(\n"
                    + "  runtime_id: \"runtime-a\",\n"
                    + "  pipeline_kind: \"http/request\",\n"
                    + "  target: \"pid:42\",\n"
                    + ")\n"
            || Deploy("runtime-a", "http/request", null)
                != "fn main() = runtime.deploy(\n"
                    + "  runtime_id: \"runtime-a\",\n"
                    + "  pipeline_kind: \"http/request\",\n"
                    + "  target: none,\n"
                    + ")\n"
            || Deploy("runtime-a", "http/request", "label:\"a\"\\b")
                != "fn main() = runtime.deploy(\n"
                    + "  runtime_id: \"runtime-a\",\n"
                    + "  pipeline_kind: \"http/request\",\n"
                    + "  target: \"label:\\\"a\\\"\\\\b\",\n"
                    + ")\n")
        {
            throw new InvalidDataException(
                "GUI Leselang export diverged from the canonical CLI format");
        }
        try
        {
            Deploy("runtime-a", "bad kind", null);
            throw new InvalidDataException(
                "GUI Leselang export accepted an invalid deployment");
        }
        catch (ArgumentException)
        {
        }
    }

    private static string Quote(string value)
    {
        var output = new StringBuilder(value.Length + 2);
        output.Append('"');
        foreach (var character in value)
        {
            output.Append(character switch
            {
                '"' => "\\\"",
                '\\' => "\\\\",
                '\n' => "\\n",
                '\r' => "\\r",
                '\t' => "\\t",
                _ => character.ToString(),
            });
        }
        output.Append('"');
        return output.ToString();
    }

    private static string Bounded(string source)
    {
        if (source.Length > MaxSourceLength)
        {
            throw new InvalidDataException("Leselang export exceeds the source limit");
        }
        return source;
    }
}
