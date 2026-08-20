using System.Globalization;
using System.Text;

public enum RemoteMutationFailureKind
{
    RemoteRejected,
    InvalidRequest,
    InvalidResponse,
    Timeout,
    Transport,
    Unexpected,
    OwnerCancelled,
    StaleOperation,
}

public enum RemoteMutationFailureDisposition
{
    KnownRejection,
    UnknownOutcome,
    Cancelled,
    Ignored,
}

public sealed record RemoteMutationFailure(
    RemoteMutationFailureKind Kind,
    RemoteMutationFailureDisposition Disposition,
    string? Code,
    string? Detail,
    string? OperatorMessage)
{
    public bool RequiresOperatorAttention => OperatorMessage is not null;
}

public static class RemoteMutationFailurePolicy
{
    public const int MaxCodeLength = 64;
    public const int MaxDetailLength = 180;
    public const int MaxOperatorMessageLength = 320;

    public static RemoteMutationFailure Classify(
        RemoteMutationKind mutationKind,
        Exception error,
        bool ownerCancellationRequested)
    {
        ArgumentNullException.ThrowIfNull(error);
        if (!Enum.IsDefined(mutationKind))
        {
            throw new ArgumentOutOfRangeException(nameof(mutationKind));
        }
        if (ownerCancellationRequested)
        {
            return Silent(
                RemoteMutationFailureKind.OwnerCancelled,
                RemoteMutationFailureDisposition.Cancelled);
        }

        var label = Label(mutationKind);
        return error switch
        {
            RemoteMutationException remote => RemoteRejection(label, remote),
            ArgumentException invalid => Visible(
                RemoteMutationFailureKind.InvalidRequest,
                RemoteMutationFailureDisposition.KnownRejection,
                null,
                Display(invalid.Message, "request validation failed"),
                detail => $"{label} blocked: {detail}"),
            InvalidDataException invalid => Visible(
                RemoteMutationFailureKind.InvalidResponse,
                RemoteMutationFailureDisposition.UnknownOutcome,
                null,
                Display(invalid.Message, "response validation failed"),
                detail => $"{label} outcome unknown after an invalid response ({detail}); wait for an authoritative snapshot before retrying"),
            OperationCanceledException => Visible(
                RemoteMutationFailureKind.Timeout,
                RemoteMutationFailureDisposition.UnknownOutcome,
                null,
                null,
                _ => $"{label} outcome unknown after timeout; wait for an authoritative snapshot before retrying"),
            HttpRequestException => Visible(
                RemoteMutationFailureKind.Transport,
                RemoteMutationFailureDisposition.UnknownOutcome,
                null,
                null,
                _ => $"{label} outcome unknown after a network failure; wait for an authoritative snapshot before retrying"),
            _ => Visible(
                RemoteMutationFailureKind.Unexpected,
                RemoteMutationFailureDisposition.UnknownOutcome,
                null,
                null,
                _ => $"{label} outcome unknown after an unexpected local failure; wait for an authoritative snapshot before retrying"),
        };
    }

    internal static RemoteMutationFailure StaleOperation() => Silent(
        RemoteMutationFailureKind.StaleOperation,
        RemoteMutationFailureDisposition.Ignored);

    public static void VerifyContract()
    {
        var rejected = Classify(
            RemoteMutationKind.Refresh,
            new RemoteMutationException("revision\nconflict", "stale\r\nrevision"),
            ownerCancellationRequested: false);
        Require(
            rejected is
            {
                Kind: RemoteMutationFailureKind.RemoteRejected,
                Disposition: RemoteMutationFailureDisposition.KnownRejection,
                Code: "revisionconflict",
                Detail: "stalerevision",
                RequiresOperatorAttention: true,
            },
            "remote rejection classification drifted");

        var invalid = Classify(
            RemoteMutationKind.CapabilityRefresh,
            new InvalidDataException("identity mismatch"),
            ownerCancellationRequested: false);
        Require(
            invalid.Kind == RemoteMutationFailureKind.InvalidResponse
                && invalid.Disposition == RemoteMutationFailureDisposition.UnknownOutcome
                && invalid.OperatorMessage?.StartsWith(
                    "Capability discovery outcome unknown",
                    StringComparison.Ordinal) == true,
            "invalid response was not classified as an unknown outcome");

        var timeout = Classify(
            RemoteMutationKind.Deployment,
            new TaskCanceledException("private timeout detail"),
            ownerCancellationRequested: false);
        Require(
            timeout.Kind == RemoteMutationFailureKind.Timeout
                && timeout.Detail is null
                && timeout.OperatorMessage?.Contains("private timeout detail", StringComparison.Ordinal)
                    == false,
            "timeout classification exposed transport detail");

        var transport = Classify(
            RemoteMutationKind.Deployment,
            new HttpRequestException("https://private.example.invalid/?token=secret"),
            ownerCancellationRequested: false);
        Require(
            transport.Kind == RemoteMutationFailureKind.Transport
                && transport.Disposition == RemoteMutationFailureDisposition.UnknownOutcome
                && transport.OperatorMessage?.Contains("private.example", StringComparison.Ordinal)
                    == false,
            "transport classification exposed endpoint detail");

        var unexpected = Classify(
            RemoteMutationKind.Refresh,
            new InvalidOperationException("credential=do-not-render"),
            ownerCancellationRequested: false);
        Require(
            unexpected.Kind == RemoteMutationFailureKind.Unexpected
                && unexpected.Detail is null
                && unexpected.OperatorMessage?.Contains("do-not-render", StringComparison.Ordinal)
                    == false,
            "unexpected failure exposed exception detail");

        var cancelled = Classify(
            RemoteMutationKind.Refresh,
            new HttpRequestException("shutdown race"),
            ownerCancellationRequested: true);
        Require(
            cancelled is
            {
                Kind: RemoteMutationFailureKind.OwnerCancelled,
                Disposition: RemoteMutationFailureDisposition.Cancelled,
                RequiresOperatorAttention: false,
            },
            "owner cancellation was reclassified by its incidental exception");

        var bounded = Classify(
            RemoteMutationKind.Deployment,
            new RemoteMutationException(
                new string('x', MaxCodeLength * 2),
                new string('y', MaxDetailLength * 2)),
            ownerCancellationRequested: false);
        Require(
            bounded.Code?.Length == MaxCodeLength
                && bounded.Detail?.Length == MaxDetailLength
                && bounded.OperatorMessage?.Length <= MaxOperatorMessageLength,
            "mutation failure diagnostics exceeded their bounds");
    }

    private static RemoteMutationFailure RemoteRejection(
        string label,
        RemoteMutationException error)
    {
        var code = Code(error.Code);
        var detail = Display(error.Message, "remote request rejected");
        return Visible(
            RemoteMutationFailureKind.RemoteRejected,
            RemoteMutationFailureDisposition.KnownRejection,
            code,
            detail,
            value => $"{label} rejected ({code}): {value}");
    }

    private static RemoteMutationFailure Visible(
        RemoteMutationFailureKind kind,
        RemoteMutationFailureDisposition disposition,
        string? code,
        string? detail,
        Func<string?, string> message)
    {
        var operatorMessage = message(detail);
        if (operatorMessage.Length > MaxOperatorMessageLength)
        {
            throw new InvalidDataException(
                "mutation failure operator message exceeds its fixed bound");
        }
        return new RemoteMutationFailure(kind, disposition, code, detail, operatorMessage);
    }

    private static RemoteMutationFailure Silent(
        RemoteMutationFailureKind kind,
        RemoteMutationFailureDisposition disposition) => new(
        kind,
        disposition,
        null,
        null,
        null);

    private static string Label(RemoteMutationKind kind) => kind switch
    {
        RemoteMutationKind.Refresh => "Refresh",
        RemoteMutationKind.CapabilityRefresh => "Capability discovery",
        RemoteMutationKind.Deployment => "Deployment",
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };

    private static string Code(string value)
    {
        var safe = new string(value
            .Where(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '_' or '-')
            .Take(MaxCodeLength)
            .ToArray());
        return safe.Length == 0 ? "remote_rejected" : safe;
    }

    private static string Display(string value, string fallback)
    {
        var result = new StringBuilder();
        var count = 0;
        foreach (var rune in value.EnumerateRunes())
        {
            if (count >= MaxDetailLength)
            {
                break;
            }
            var category = Rune.GetUnicodeCategory(rune);
            if (category is UnicodeCategory.Control
                or UnicodeCategory.Format
                or UnicodeCategory.LineSeparator
                or UnicodeCategory.ParagraphSeparator
                or UnicodeCategory.PrivateUse
                or UnicodeCategory.Surrogate
                or UnicodeCategory.OtherNotAssigned)
            {
                continue;
            }
            result.Append(rune.ToString());
            count += 1;
        }
        var safe = result.ToString().Trim();
        return safe.Length == 0 ? fallback : safe;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidDataException(message);
        }
    }
}
