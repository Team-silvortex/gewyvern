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

        var submitStart = index.IndexOf("<button id=\"register-submit\"", StringComparison.Ordinal);
        Assert.True(submitStart >= 0);
        var submitEnd = index.IndexOf('>', submitStart);
        Assert.True(submitEnd > submitStart);

        var submitTag = index[submitStart..submitEnd];
        Assert.Contains("type=\"submit\"", submitTag, StringComparison.Ordinal);
        Assert.Contains("data-i18n=\"register.submit\"", submitTag, StringComparison.Ordinal);
        Assert.Contains("aria-describedby=\"register-guidance\"", submitTag, StringComparison.Ordinal);
        Assert.Contains(" disabled", submitTag, StringComparison.Ordinal);
    }

    [Fact]
    public void PublishedUiGuardsImportsCleanupAndDuplicateSubmission()
    {
        var app = File.ReadAllText(AssetPath("app.js"));

        Assert.Contains("file.size > 1_048_576", app, StringComparison.Ordinal);
        Assert.Contains("persistence.importConfirm", app, StringComparison.Ordinal);
        Assert.Contains("state.uiActions.has(\"register-runtime\")", app, StringComparison.Ordinal);
        Assert.Contains("/v1/runtimes/registration-plan", app, StringComparison.Ordinal);
        Assert.Contains("registrationPlanToken: registrationPlan.planToken", app, StringComparison.Ordinal);
        Assert.DoesNotContain("function findDuplicateRuntime(", app, StringComparison.Ordinal);
        var planDraftStart = app.IndexOf("function registrationPlanDraft()", StringComparison.Ordinal);
        var planDraftEnd = app.IndexOf("function registrationPlanDraftKey(", planDraftStart, StringComparison.Ordinal);
        Assert.True(planDraftStart >= 0 && planDraftEnd > planDraftStart);
        var planDraft = app[planDraftStart..planDraftEnd];
        Assert.DoesNotContain("registerToken", planDraft, StringComparison.Ordinal);
        Assert.DoesNotContain("registerSidecarAdminToken", planDraft, StringComparison.Ordinal);
        Assert.Contains("`/v1/runtimes/${runtimeId}/recovery`", app, StringComparison.Ordinal);
        Assert.Contains("const kind = action.commandKind", app, StringComparison.Ordinal);
        Assert.DoesNotContain("function recoveryActionKind(", app, StringComparison.Ordinal);
        Assert.Contains("state.cache.cleanupPlan?.failed?.runtimeCount", app, StringComparison.Ordinal);
        Assert.Contains("state.cache.cleanupPlan?.unobserved?.runtimeCount", app, StringComparison.Ordinal);
        Assert.Contains("state.cache.cleanupPlan?.slice?.runtimeCount", app, StringComparison.Ordinal);
        Assert.Contains("/v1/runtimes/cleanup-plan", app, StringComparison.Ordinal);
        Assert.Contains("planToken: plan.planToken", app, StringComparison.Ordinal);
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
