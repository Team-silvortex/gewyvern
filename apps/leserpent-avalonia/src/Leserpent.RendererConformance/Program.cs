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

Console.WriteLine($"renderer conformance valid: revision={renderer.Document.Revision}");
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
