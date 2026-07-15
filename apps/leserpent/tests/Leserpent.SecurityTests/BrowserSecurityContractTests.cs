using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Http;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class BrowserSecurityContractTests
{
    [Fact]
    public void BrowserHeadersBlockEmbeddingAndRestrictActiveContent()
    {
        var context = new DefaultHttpContext();

        BrowserSecurityHeaders.Apply(context.Response);

        Assert.Equal("DENY", context.Response.Headers["X-Frame-Options"]);
        Assert.Equal("nosniff", context.Response.Headers["X-Content-Type-Options"]);
        Assert.Equal("no-referrer", context.Response.Headers["Referrer-Policy"]);
        var policy = context.Response.Headers["Content-Security-Policy"].ToString();
        Assert.Contains("frame-ancestors 'none'", policy);
        Assert.Contains("script-src 'self'", policy);
        Assert.Contains("object-src 'none'", policy);
    }

    [Fact]
    public void EveryPublishedRuntimeFrameIsSandboxedWithoutPermissions()
    {
        var index = File.ReadAllText(AssetPath("index.html"));
        var app = File.ReadAllText(AssetPath("app.js"));

        Assert.Contains("referrerpolicy=\"no-referrer\" sandbox", index, StringComparison.Ordinal);
        Assert.Contains("referrerpolicy=\"no-referrer\" sandbox data-runtime-window-frame", app, StringComparison.Ordinal);
        Assert.DoesNotContain("sandbox=\"allow-", index, StringComparison.Ordinal);
        Assert.DoesNotContain("sandbox=\"allow-", app, StringComparison.Ordinal);
    }

    [Fact]
    public void PublishedLanguagePackDownloadsUseBoundedStreamingReads()
    {
        var app = File.ReadAllText(AssetPath("app.js"));

        Assert.Contains("response.body.getReader()", app, StringComparison.Ordinal);
        Assert.Contains("catalogBytes: 128 * 1024", app, StringComparison.Ordinal);
        Assert.Contains("catalogPacks: 64", app, StringComparison.Ordinal);
        Assert.DoesNotContain("const catalog = await response.json()", app, StringComparison.Ordinal);
        Assert.DoesNotContain("const text = await response.text()", app, StringComparison.Ordinal);
    }

    private static string AssetPath(string fileName) =>
        Path.Combine(AppContext.BaseDirectory, "wwwroot", fileName);
}
