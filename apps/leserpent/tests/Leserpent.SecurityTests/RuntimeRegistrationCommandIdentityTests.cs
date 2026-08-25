using Leserpent.ControlPlane;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeRegistrationCommandIdentityTests
{
    [Fact]
    public void ExactNormalizedRetryKeepsTheSameIdentity()
    {
        var first = RuntimeRegistrationCommandIdentity.ForIntent(
            "runtime-a",
            " Runtime A ",
            " https://runtime.example ",
            " https://sidecar.example ",
            new RuntimeTags(" prod ", " eu ", " edge "),
            7);
        var retry = RuntimeRegistrationCommandIdentity.ForIntent(
            "runtime-a",
            "Runtime A",
            "https://runtime.example",
            "https://sidecar.example",
            new RuntimeTags("prod", "eu", "edge"),
            7);

        Assert.Equal(first, retry);
        Assert.Matches("^[0-9a-f]{32}$", first);
    }

    [Fact]
    public void ReviewedRevisionRotatesUpdateIdentity()
    {
        var create = Identity(expectedRevision: null);
        var firstUpdate = Identity(expectedRevision: 7);
        var retriedUpdate = Identity(expectedRevision: 7);
        var laterUpdate = Identity(expectedRevision: 8);

        Assert.Equal(firstUpdate, retriedUpdate);
        Assert.NotEqual(create, firstUpdate);
        Assert.NotEqual(firstUpdate, laterUpdate);
    }

    [Fact]
    public void EveryRegistrationCommandFieldParticipatesInIdentity()
    {
        var baseline = Identity(expectedRevision: 7);
        var variants = new[]
        {
            RuntimeRegistrationCommandIdentity.ForIntent(
                "runtime-b",
                "Runtime A",
                "https://runtime.example",
                "https://sidecar.example",
                new RuntimeTags("prod", "eu", "edge"),
                7),
            RuntimeRegistrationCommandIdentity.ForIntent(
                "runtime-a",
                "Runtime B",
                "https://runtime.example",
                "https://sidecar.example",
                new RuntimeTags("prod", "eu", "edge"),
                7),
            RuntimeRegistrationCommandIdentity.ForIntent(
                "runtime-a",
                "Runtime A",
                "https://runtime-b.example",
                "https://sidecar.example",
                new RuntimeTags("prod", "eu", "edge"),
                7),
            RuntimeRegistrationCommandIdentity.ForIntent(
                "runtime-a",
                "Runtime A",
                "https://runtime.example",
                "https://sidecar-b.example",
                new RuntimeTags("prod", "eu", "edge"),
                7),
            RuntimeRegistrationCommandIdentity.ForIntent(
                "runtime-a",
                "Runtime A",
                "https://runtime.example",
                "https://sidecar.example",
                new RuntimeTags("staging", "eu", "edge"),
                7),
            RuntimeRegistrationCommandIdentity.ForIntent(
                "runtime-a",
                "Runtime A",
                "https://runtime.example",
                "https://sidecar.example",
                new RuntimeTags("prod", "us", "edge"),
                7),
            RuntimeRegistrationCommandIdentity.ForIntent(
                "runtime-a",
                "Runtime A",
                "https://runtime.example",
                "https://sidecar.example",
                new RuntimeTags("prod", "eu", "control"),
                7),
        };

        Assert.All(variants, identity => Assert.NotEqual(baseline, identity));
        Assert.Equal(variants.Length, variants.Distinct(StringComparer.Ordinal).Count());
    }

    [Fact]
    public void CanonicalEncodingPreservesFieldBoundaries()
    {
        var first = RuntimeRegistrationCommandIdentity.ForIntent(
            "runtime-a",
            "a|b",
            "c",
            null,
            null,
            null);
        var second = RuntimeRegistrationCommandIdentity.ForIntent(
            "runtime-a",
            "a",
            "b|c",
            null,
            null,
            null);

        Assert.NotEqual(first, second);
    }

    private static string Identity(ulong? expectedRevision) =>
        RuntimeRegistrationCommandIdentity.ForIntent(
            "runtime-a",
            "Runtime A",
            "https://runtime.example",
            "https://sidecar.example",
            new RuntimeTags("prod", "eu", "edge"),
            expectedRevision);
}
