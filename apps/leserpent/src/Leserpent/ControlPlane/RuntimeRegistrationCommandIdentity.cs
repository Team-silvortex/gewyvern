using System.Security.Cryptography;
using System.Text.Json;

namespace Leserpent.ControlPlane;

internal static class RuntimeRegistrationCommandIdentity
{
    private const int IdentitySchemaVersion = 1;

    internal static string ForIntent(
        string runtimeId,
        string name,
        string endpoint,
        string? sidecarEndpoint,
        RuntimeTags? tags,
        ulong? expectedRevision)
    {
        var normalizedTags = tags ?? new RuntimeTags(null, null, null);
        using var output = new MemoryStream();
        using (var writer = new Utf8JsonWriter(output))
        {
            writer.WriteStartObject();
            writer.WriteString("domain", "leserpent.runtime-registration");
            writer.WriteNumber("identity_schema", IdentitySchemaVersion);
            writer.WriteString(
                "command_kind",
                expectedRevision is null
                    ? "runtime_register"
                    : "runtime_registration_update");
            writer.WriteString("runtime_id", runtimeId);
            if (expectedRevision is null)
            {
                writer.WriteNull("expected_revision");
            }
            else
            {
                writer.WriteNumber("expected_revision", expectedRevision.Value);
            }
            writer.WriteString("name", name.Trim());
            writer.WriteString("endpoint", endpoint.Trim());
            WriteOptionalString(
                writer,
                "sidecar_endpoint",
                sidecarEndpoint?.Trim());
            writer.WritePropertyName("tags");
            writer.WriteStartObject();
            WriteOptionalString(
                writer,
                "environment",
                normalizedTags.Environment?.Trim());
            WriteOptionalString(
                writer,
                "cluster",
                normalizedTags.Cluster?.Trim());
            WriteOptionalString(
                writer,
                "role",
                normalizedTags.Role?.Trim());
            writer.WriteEndObject();
            writer.WriteEndObject();
        }

        var digest = SHA256.HashData(output.ToArray());
        return Convert.ToHexString(digest.AsSpan(0, 16)).ToLowerInvariant();
    }

    private static void WriteOptionalString(
        Utf8JsonWriter writer,
        string name,
        string? value)
    {
        if (value is null)
        {
            writer.WriteNull(name);
        }
        else
        {
            writer.WriteString(name, value);
        }
    }
}
