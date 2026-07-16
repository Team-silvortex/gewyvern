using Avalonia;

internal static class Program
{
    [STAThread]
    public static int Main(string[] args) => BuildAvaloniaApp()
        .StartWithClassicDesktopLifetime(args);

    public static AppBuilder BuildAvaloniaApp() => AppBuilder
        .Configure<LeserpentApp>()
        .UsePlatformDetect()
        .LogToTrace();
}
