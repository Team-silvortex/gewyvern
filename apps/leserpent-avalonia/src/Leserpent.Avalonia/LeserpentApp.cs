using System.Text.Json;
using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Styling;
using Avalonia.Themes.Fluent;
using Avalonia.Threading;

internal sealed class LeserpentApp : Application
{
    private const int MaxPayloadBytes = 2 * 1024 * 1024;

    public override void Initialize()
    {
        RequestedThemeVariant = ThemeVariant.Dark;
        Styles.Add(new FluentTheme());
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            var verifyControls = desktop.Args is ["--verify-controls", _];
            var document = LoadDocument(desktop.Args, verifyControls);
            var window = new MainWindow(document);
            desktop.MainWindow = window;
            if (verifyControls)
            {
                window.Opened += (_, _) =>
                {
                    Console.WriteLine(
                        $"Avalonia controls valid: nodes={window.RenderedNodeCount}, revision={document.Revision}");
                    DispatcherTimer.RunOnce(
                        () => desktop.Shutdown(0),
                        TimeSpan.FromMilliseconds(100));
                };
            }
        }
        base.OnFrameworkInitializationCompleted();
    }

    private static UiDocument LoadDocument(string[]? args, bool verifyControls)
    {
        var fixturePath = args switch
        {
            [var path] when !verifyControls => path,
            ["--verify-controls", var path] when verifyControls => path,
            _ => throw new InvalidDataException(
                "usage: Leserpent.Avalonia [--verify-controls] FIXTURE"),
        };
        if (string.IsNullOrWhiteSpace(fixturePath))
        {
            throw new InvalidDataException("fixture path is empty");
        }

        using var stream = new FileStream(
            fixturePath,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read);
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

        var fixture = JsonSerializer.Deserialize<RendererFixture>(payload, RendererJson.CreateOptions())
            ?? throw new InvalidDataException("fixture is empty");
        if (fixture.SchemaVersion != 1)
        {
            throw new InvalidDataException("unsupported fixture schema");
        }

        var renderer = new SemanticRenderer();
        renderer.Mount(fixture.Previous);
        renderer.Apply(fixture.Patch);
        return renderer.Document;
    }
}
