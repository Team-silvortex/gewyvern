using Xunit;

namespace Leserpent.SecurityTests;

public sealed class OperatorGuardrailContractTests
{
    [Fact]
    public void RegistrationFormUsesNativeConstraintsAndStartsBlocked()
    {
        var index = File.ReadAllText(AssetPath("index.html"));

        Assert.Contains("id=\"register-endpoint\" type=\"url\"", index, StringComparison.Ordinal);
        Assert.Contains("id=\"register-token\" type=\"password\"", index, StringComparison.Ordinal);
        Assert.Contains("id=\"register-submit\" type=\"submit\"", index, StringComparison.Ordinal);
        Assert.Contains("data-i18n=\"register.submit\" disabled", index, StringComparison.Ordinal);
    }

    [Fact]
    public void PublishedUiGuardsImportsCleanupAndDuplicateSubmission()
    {
        var app = File.ReadAllText(AssetPath("app.js"));

        Assert.Contains("file.size > 1_048_576", app, StringComparison.Ordinal);
        Assert.Contains("persistence.importConfirm", app, StringComparison.Ordinal);
        Assert.Contains("state.uiActions.has(\"register-runtime\")", app, StringComparison.Ordinal);
        Assert.Contains("currentFailedRuntimeCount() === 0", app, StringComparison.Ordinal);
        Assert.Contains("currentUnobservedRuntimeCount() === 0", app, StringComparison.Ordinal);
        Assert.Contains("currentSliceCount() === 0", app, StringComparison.Ordinal);
        Assert.Contains("runUiActionOnce(\"fleet-refresh\"", app, StringComparison.Ordinal);
        Assert.Contains("`runtime-refresh:${runtimeId}`", app, StringComparison.Ordinal);
        Assert.Contains("runtimeClearSliceChallenge", app, StringComparison.Ordinal);
        Assert.Contains("Already completed steps cannot be rolled back", app, StringComparison.Ordinal);
        Assert.Contains("languagePacks.removeConfirm", app, StringComparison.Ordinal);
        Assert.Contains("runUiActionOnce(\"language-pack-import\"", app, StringComparison.Ordinal);
        Assert.Contains("runUiActionOnce(\"persistence-save\"", app, StringComparison.Ordinal);
        Assert.Contains("runUiActionOnce(\"persistence-export\"", app, StringComparison.Ordinal);
        Assert.Contains("function syncFilterActionState()", app, StringComparison.Ordinal);
        Assert.Contains("event.key === \"Enter\"", app, StringComparison.Ordinal);
    }

    private static string AssetPath(string fileName) =>
        Path.Combine(AppContext.BaseDirectory, "wwwroot", fileName);
}
