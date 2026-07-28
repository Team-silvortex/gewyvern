using System.Text.Json;
using System.Text.Json.Nodes;

const int MaxPayloadBytes = 2 * 1024 * 1024;

if (args.Length != 1)
{
    Console.Error.WriteLine("usage: Leserpent.RendererConformance FIXTURE");
    return 2;
}

var payload = ReadBoundedFixture(args[0]);
var fixture = JsonSerializer.Deserialize(
    payload,
    RendererJsonContext.Default.RendererFixture)
    ?? throw new InvalidDataException("fixture is empty");
if (fixture.SchemaVersion != 1)
{
    throw new InvalidDataException("unsupported fixture schema");
}

var renderer = new SemanticRenderer();
renderer.Mount(fixture.Previous);
renderer.Apply(fixture.Patch);
var actual = JsonSerializer.SerializeToNode(
    renderer.Document,
    RendererJsonContext.Default.UiDocument);
var expected = JsonSerializer.SerializeToNode(
    fixture.Next,
    RendererJsonContext.Default.UiDocument);
if (!JsonNode.DeepEquals(actual, expected))
{
    throw new InvalidDataException("incremental render does not match the next document");
}

if (fixture.PresentationOperation is not { } operation)
{
    Console.WriteLine($"renderer conformance valid: revision={renderer.Document.Revision}");
    return 0;
}

var operationPayload = JsonSerializer.SerializeToUtf8Bytes(
    operation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedOperation = JsonSerializer.Deserialize(
    operationPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("presentation operation round trip failed");
if (renderer.ValidatePresentationOperation(decodedOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.Focus,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.Focus,
        NodeId = renderer.Document.Root.Id,
    }) != UiPresentationValidation.UnfocusableTarget)
{
    throw new InvalidDataException("presentation operation validation diverged");
}

Console.WriteLine(
    $"renderer conformance valid: revision={renderer.Document.Revision}, presentation_focus=true, strict_codec=true");
return 0;

static byte[] ReadBoundedFixture(string path)
{
    using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
    if (stream.Length > MaxPayloadBytes)
    {
        throw new InvalidDataException("fixture exceeds the UI IR payload limit");
    }

    var payload = new byte[checked((int)stream.Length)];
    stream.ReadExactly(payload);
    if (stream.ReadByte() != -1)
    {
        throw new InvalidDataException("fixture changed while being read");
    }
    return payload;
}
