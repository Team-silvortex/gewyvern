using Xunit;

namespace Leserpent.SecurityTests;

public sealed class CrashHarnessAssemblyLocatorTests
{
    [Fact]
    public void ExplicitIsolatedArtifactPathMustBeAbsoluteAndIdentityBound()
    {
        var expected = Path.Combine(
            Path.GetTempPath(),
            "isolated-dotnet-artifacts",
            "Leserpent.RuntimeDeletionCrashHarness.dll");

        Assert.Equal(
            Path.GetFullPath(expected),
            CrashHarnessAssemblyLocator.Resolve(
                expected,
                AppContext.BaseDirectory));
        Assert.Throws<InvalidDataException>(() =>
            CrashHarnessAssemblyLocator.Resolve(
                Path.Combine(
                    "relative",
                    "Leserpent.RuntimeDeletionCrashHarness.dll"),
                AppContext.BaseDirectory));
        Assert.Throws<InvalidDataException>(() =>
            CrashHarnessAssemblyLocator.Resolve(
                Path.Combine(
                    Path.GetTempPath(),
                    "UnexpectedHarness.dll"),
                AppContext.BaseDirectory));
    }
}
