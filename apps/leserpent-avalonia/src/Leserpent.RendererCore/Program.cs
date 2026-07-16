using System.Text.Json;
using System.Text.Json.Serialization;

public sealed class SemanticRenderer
{
    private const int MaxPatchOperations = 8192;

    public UiDocument Document { get; private set; } = null!;

    public void Mount(UiDocument document)
    {
        ValidateDocument(document);
        Document = Clone(document);
    }

    public void Apply(UiPatch patch)
    {
        if (patch.SchemaVersion != 1 || patch.FromRevision != Document.Revision)
        {
            throw new InvalidDataException("patch revision or schema mismatch");
        }
        if (patch.ToRevision < patch.FromRevision || patch.Operations.Count > MaxPatchOperations)
        {
            throw new InvalidDataException("invalid patch bounds");
        }

        var previous = Document;
        Document = Clone(Document);
        try
        {
            foreach (var operation in patch.Operations)
            {
                ApplyOperation(operation);
            }
            Document.Revision = patch.ToRevision;
            ValidateDocument(Document);
        }
        catch
        {
            Document = previous;
            throw;
        }
    }

    private void ApplyOperation(UiPatchOperation operation)
    {
        switch (operation.Kind)
        {
            case PatchKind.Remove:
                Require(operation.NodeId is not null, "remove target is missing");
                Require(operation.ParentId is null && operation.Index is null && operation.Node is null,
                    "remove contains unrelated fields");
                Require(operation.NodeId != Document.Root.Id, "root cannot be removed");
                Require(Remove(Document.Root, operation.NodeId!) is not null, "remove target not found");
                break;
            case PatchKind.Insert:
                Require(operation.ParentId is not null && operation.Index is not null && operation.Node is not null,
                    "insert payload is incomplete");
                Require(operation.NodeId is null, "insert contains unrelated fields");
                var insertIndex = operation.Index
                    ?? throw new InvalidDataException("insert index is missing");
                Require(Find(Document.Root, operation.Node!.Id) is null, "insert ID already exists");
                var insertParent = Find(Document.Root, operation.ParentId!)
                    ?? throw new InvalidDataException("insert parent not found");
                Require(insertIndex <= insertParent.Children.Count, "insert index is invalid");
                insertParent.Children.Insert(insertIndex, Clone(operation.Node));
                break;
            case PatchKind.Move:
                Require(operation.NodeId is not null && operation.ParentId is not null && operation.Index is not null,
                    "move payload is incomplete");
                Require(operation.Node is null, "move contains unrelated fields");
                var moveIndex = operation.Index
                    ?? throw new InvalidDataException("move index is missing");
                var moving = Find(Document.Root, operation.NodeId!)
                    ?? throw new InvalidDataException("move target not found");
                Require(Find(moving, operation.ParentId!) is null, "cyclic move");
                moving = Remove(Document.Root, operation.NodeId!)
                    ?? throw new InvalidDataException("move target disappeared");
                var moveParent = Find(Document.Root, operation.ParentId!)
                    ?? throw new InvalidDataException("move parent not found");
                Require(moveIndex <= moveParent.Children.Count, "move index is invalid");
                moveParent.Children.Insert(moveIndex, moving);
                break;
            case PatchKind.Update:
                Require(operation.Node is not null && operation.Node.Children.Count == 0,
                    "update must contain one shallow node");
                Require(operation.NodeId is null && operation.ParentId is null && operation.Index is null,
                    "update contains unrelated fields");
                var target = Find(Document.Root, operation.Node!.Id)
                    ?? throw new InvalidDataException("update target not found");
                target.Kind = operation.Node.Kind;
                target.RuntimeId = operation.Node.RuntimeId;
                target.DebuggerSessionId = operation.Node.DebuggerSessionId;
                target.Text = operation.Node.Text;
                target.Accessibility = operation.Node.Accessibility;
                target.Action = operation.Node.Action;
                break;
            default:
                throw new InvalidDataException("unknown patch operation");
        }
    }

    private static void ValidateDocument(UiDocument document)
    {
        Require(document.SchemaVersion == 1, "unsupported document schema");
        var ids = new HashSet<string>(StringComparer.Ordinal);
        ValidateNode(document.Root, 1, null, null, ids);
        Require(ids.Count <= 4096, "document exceeds the node limit");
    }

    private static void ValidateNode(
        UiNode node,
        int depth,
        string? runtimeContext,
        string? debuggerContext,
        HashSet<string> ids)
    {
        Require(depth <= 32, "document exceeds the depth limit");
        Require(IsIdentifier(node.Id) && ids.Add(node.Id), "invalid or duplicate node ID");
        ValidateText(node.Text);
        ValidateText(node.Accessibility.Label);
        ValidateText(node.Accessibility.Description);
        Require(node.Action is null || node.Accessibility.Label is not null, "action has no accessibility label");
        if (node.Kind is UiNodeKind.RuntimeCard or UiNodeKind.RuntimeWorkspace)
        {
            Require(node.RuntimeId is not null, "runtime container has no runtime ID");
            runtimeContext = node.RuntimeId;
        }
        else
        {
            Require(node.RuntimeId is null, "non-container node carries a runtime ID");
        }
        if (node.Kind is UiNodeKind.DebuggerWorkspace)
        {
            Require(node.DebuggerSessionId is not null && IsIdentifier(node.DebuggerSessionId),
                "debugger workspace has no valid session ID");
            debuggerContext = node.DebuggerSessionId;
        }
        else
        {
            Require(node.DebuggerSessionId is null,
                "non-debugger container carries a debugger session ID");
        }
        if (node.Action is not null)
        {
            var validAction = node.Action.Kind switch
            {
                ActionKind.RuntimeInspect or ActionKind.RuntimeRefresh =>
                    node.Action.RuntimeId is not null
                    && IsIdentifier(node.Action.RuntimeId)
                    && node.Action.RuntimeId == runtimeContext
                    && node.Action.SessionId is null,
                ActionKind.DebuggerCancel => node.Action.RuntimeId is null
                    && node.Action.SessionId is not null
                    && IsIdentifier(node.Action.SessionId)
                    && node.Action.SessionId == debuggerContext,
                _ => false,
            };
            Require(validAction, "action context binding is invalid");
        }
        foreach (var child in node.Children)
        {
            ValidateNode(child, depth + 1, runtimeContext, debuggerContext, ids);
        }
    }

    private static void ValidateText(LocalizedText? text)
    {
        if (text is null) return;
        Require(IsIdentifier(text.Key) && text.Fallback.Length <= 1024
            && !text.Fallback.Any(char.IsControl), "invalid localized text");
    }

    private static bool IsIdentifier(string value) => value.Length is > 0 and <= 128
        && value.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '-' or '_' or '.' or ':');

    private static UiNode? Find(UiNode node, string id)
    {
        if (node.Id == id) return node;
        foreach (var child in node.Children)
        {
            var found = Find(child, id);
            if (found is not null) return found;
        }
        return null;
    }

    private static UiNode? Remove(UiNode node, string id)
    {
        var index = node.Children.FindIndex(child => child.Id == id);
        if (index >= 0)
        {
            var removed = node.Children[index];
            node.Children.RemoveAt(index);
            return removed;
        }
        foreach (var child in node.Children)
        {
            var removed = Remove(child, id);
            if (removed is not null) return removed;
        }
        return null;
    }

    private static UiDocument Clone(UiDocument document) => new()
    {
        SchemaVersion = document.SchemaVersion,
        Revision = document.Revision,
        Root = Clone(document.Root),
    };

    private static UiNode Clone(UiNode node) => new()
    {
        Id = node.Id,
        Kind = node.Kind,
        RuntimeId = node.RuntimeId,
        DebuggerSessionId = node.DebuggerSessionId,
        Text = Clone(node.Text),
        Accessibility = new Accessibility
        {
            Label = Clone(node.Accessibility.Label),
            Description = Clone(node.Accessibility.Description),
        },
        Action = node.Action is null ? null : new UiAction
        {
            Kind = node.Action.Kind,
            RuntimeId = node.Action.RuntimeId,
            SessionId = node.Action.SessionId,
        },
        Children = node.Children.Select(Clone).ToList(),
    };

    private static LocalizedText? Clone(LocalizedText? text) => text is null ? null : new()
    {
        Key = text.Key,
        Fallback = text.Fallback,
    };

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidDataException(message);
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RendererFixture
{
    public int SchemaVersion { get; set; }
    public required UiDocument Previous { get; set; }
    public required UiPatch Patch { get; set; }
    public required UiDocument Next { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiDocument
{
    public int SchemaVersion { get; set; }
    public ulong Revision { get; set; }
    public required UiNode Root { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiNode
{
    public required string Id { get; set; }
    public UiNodeKind Kind { get; set; }
    public string? RuntimeId { get; set; }
    public string? DebuggerSessionId { get; set; }
    public LocalizedText? Text { get; set; }
    public required Accessibility Accessibility { get; set; }
    public UiAction? Action { get; set; }
    public required List<UiNode> Children { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class LocalizedText
{
    public required string Key { get; set; }
    public required string Fallback { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class Accessibility
{
    public LocalizedText? Label { get; set; }
    public LocalizedText? Description { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiAction
{
    public ActionKind Kind { get; set; }
    public string? RuntimeId { get; set; }
    public string? SessionId { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiPatch
{
    public int SchemaVersion { get; set; }
    public ulong FromRevision { get; set; }
    public ulong ToRevision { get; set; }
    public required List<UiPatchOperation> Operations { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiPatchOperation
{
    public PatchKind Kind { get; set; }
    public string? NodeId { get; set; }
    public string? ParentId { get; set; }
    public int? Index { get; set; }
    public UiNode? Node { get; set; }
}

[JsonConverter(typeof(JsonStringEnumConverter<UiNodeKind>))]
public enum UiNodeKind
{
    [JsonStringEnumMemberName("column")] Column,
    [JsonStringEnumMemberName("heading")] Heading,
    [JsonStringEnumMemberName("text")] Text,
    [JsonStringEnumMemberName("runtime_card")] RuntimeCard,
    [JsonStringEnumMemberName("runtime_workspace")] RuntimeWorkspace,
    [JsonStringEnumMemberName("section")] Section,
    [JsonStringEnumMemberName("history_entry")] HistoryEntry,
    [JsonStringEnumMemberName("log_entry")] LogEntry,
    [JsonStringEnumMemberName("debugger_workspace")] DebuggerWorkspace,
    [JsonStringEnumMemberName("debugger_frame")] DebuggerFrame,
    [JsonStringEnumMemberName("action")] Action,
}

[JsonConverter(typeof(JsonStringEnumConverter<ActionKind>))]
public enum ActionKind
{
    [JsonStringEnumMemberName("runtime_inspect")] RuntimeInspect,
    [JsonStringEnumMemberName("runtime_refresh")] RuntimeRefresh,
    [JsonStringEnumMemberName("debugger_cancel")] DebuggerCancel,
}

[JsonConverter(typeof(JsonStringEnumConverter<PatchKind>))]
public enum PatchKind
{
    [JsonStringEnumMemberName("remove")] Remove,
    [JsonStringEnumMemberName("insert")] Insert,
    [JsonStringEnumMemberName("move")] Move,
    [JsonStringEnumMemberName("update")] Update,
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(RendererFixture))]
[JsonSerializable(typeof(UiDocument))]
public partial class RendererJsonContext : JsonSerializerContext;
