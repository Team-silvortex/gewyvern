namespace Leserpent.SecurityTests;

internal static class CrashHarnessAssemblyLocator
{
    internal const string EnvironmentVariable =
        "LESERPENT_TEST_CRASH_HARNESS_ASSEMBLY";
    private const string AssemblyName =
        "Leserpent.RuntimeDeletionCrashHarness.dll";

    internal static string Find() =>
        Resolve(
            Environment.GetEnvironmentVariable(EnvironmentVariable),
            AppContext.BaseDirectory);

    internal static string Resolve(
        string? configuredPath,
        string applicationBaseDirectory)
    {
        if (!string.IsNullOrWhiteSpace(configuredPath))
        {
            if (configuredPath.Length > 4096 ||
                configuredPath.Any(char.IsControl) ||
                !Path.IsPathFullyQualified(configuredPath))
            {
                throw new InvalidDataException(
                    $"{EnvironmentVariable} must be a bounded absolute path");
            }
            var fullPath = Path.GetFullPath(configuredPath);
            if (!string.Equals(
                    Path.GetFileName(fullPath),
                    AssemblyName,
                    StringComparison.Ordinal))
            {
                throw new InvalidDataException(
                    $"{EnvironmentVariable} must name {AssemblyName}");
            }
            return fullPath;
        }

        var repositoryRoot = FindRepositoryRoot(applicationBaseDirectory);
        var configuration = new DirectoryInfo(
            Path.TrimEndingDirectorySeparator(applicationBaseDirectory))
            .Parent?.Name ?? "Debug";
        return Path.Combine(
            repositoryRoot,
            "apps",
            "leserpent",
            "tests",
            "Leserpent.RuntimeDeletionCrashHarness",
            "bin",
            configuration,
            "net10.0",
            AssemblyName);
    }

    private static string FindRepositoryRoot(string applicationBaseDirectory)
    {
        for (var directory = new DirectoryInfo(applicationBaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            if (File.Exists(Path.Combine(directory.FullName, "Cargo.toml")))
            {
                return directory.FullName;
            }
        }
        throw new DirectoryNotFoundException(
            "could not locate the gewyvern repository root");
    }
}
