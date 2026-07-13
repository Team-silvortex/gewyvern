using System.Security.Cryptography;
using System.Text.Json;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class LanguagePackArtifactTests
{
    private static readonly HashSet<string> BuiltinLocales =
    [
        "en", "zh-CN", "zh-TW", "ja", "es", "de", "fr", "ko",
    ];

    private static readonly HashSet<string> DownloadableLocales =
    [
        "pt-BR", "it", "ru", "ar", "hi", "bn", "id", "ms", "th", "vi", "tr",
        "pl", "nl", "uk", "cs", "sv", "da", "no", "fi", "el", "he", "fa",
    ];

    [Fact]
    public void CatalogEntriesMatchPublishedPackMetadataAndDigest()
    {
        using var catalog = JsonDocument.Parse(File.ReadAllText(AssetPath("catalog.json")));
        Assert.Equal("leserpent.language-pack-catalog/v1", catalog.RootElement.GetProperty("schema").GetString());

        var entries = catalog.RootElement.GetProperty("packs").EnumerateArray().ToArray();
        Assert.Equal(30, catalog.RootElement.GetProperty("officialLocaleCount").GetInt32());
        Assert.Equal(BuiltinLocales.Count, catalog.RootElement.GetProperty("builtinLocaleCount").GetInt32());
        Assert.Equal(DownloadableLocales.Count, catalog.RootElement.GetProperty("downloadableLocaleCount").GetInt32());
        Assert.Equal(DownloadableLocales, entries.Select(entry => entry.GetProperty("locale").GetString()!).ToHashSet());
        foreach (var entry in entries)
        {
            var locale = entry.GetProperty("locale").GetString()!;
            var version = entry.GetProperty("version").GetString()!;
            var url = entry.GetProperty("url").GetString()!;
            Assert.StartsWith("/language-packs/", url, StringComparison.Ordinal);
            Assert.DoesNotContain("..", url, StringComparison.Ordinal);
            Assert.DoesNotContain(locale, BuiltinLocales);
            Assert.Equal("core-ui", entry.GetProperty("coverage").GetString());
            Assert.Contains(entry.GetProperty("direction").GetString(), new[] { "ltr", "rtl" });

            var packPath = AssetPath(Path.GetFileName(url));
            var bytes = File.ReadAllBytes(packPath);
            Assert.Equal(entry.GetProperty("sha256").GetString(), Convert.ToHexString(SHA256.HashData(bytes)).ToLowerInvariant());

            using var pack = JsonDocument.Parse(bytes);
            Assert.Equal("leserpent.language-pack/v1", pack.RootElement.GetProperty("schema").GetString());
            Assert.Equal(locale, pack.RootElement.GetProperty("locale").GetString());
            Assert.Equal(version, pack.RootElement.GetProperty("version").GetString());
            Assert.Equal(entry.GetProperty("direction").GetString(), pack.RootElement.GetProperty("direction").GetString());
            Assert.Equal(entry.GetProperty("coverage").GetString(), pack.RootElement.GetProperty("coverage").GetString());
            Assert.Equal(JsonValueKind.Object, pack.RootElement.GetProperty("translations").ValueKind);
        }
    }

    [Fact]
    public void PublishedTranslationsContainOnlySafeObjectKeysAndStringLeaves()
    {
        using var catalog = JsonDocument.Parse(File.ReadAllText(AssetPath("catalog.json")));
        foreach (var entry in catalog.RootElement.GetProperty("packs").EnumerateArray())
        {
            using var pack = JsonDocument.Parse(File.ReadAllBytes(AssetPath(Path.GetFileName(entry.GetProperty("url").GetString()!))));
            AssertSafeTranslations(pack.RootElement.GetProperty("translations"), 0);
        }
    }

    private static void AssertSafeTranslations(JsonElement value, int depth)
    {
        Assert.True(depth <= 12);
        Assert.Equal(JsonValueKind.Object, value.ValueKind);
        foreach (var property in value.EnumerateObject())
        {
            Assert.Matches("^[A-Za-z0-9_-]+$", property.Name);
            Assert.DoesNotContain(property.Name, new[] { "__proto__", "prototype", "constructor" });
            if (property.Value.ValueKind == JsonValueKind.String)
            {
                Assert.True(property.Value.GetString()!.Length <= 4000);
            }
            else
            {
                AssertSafeTranslations(property.Value, depth + 1);
            }
        }
    }

    private static string AssetPath(string fileName) =>
        Path.Combine(AppContext.BaseDirectory, "wwwroot", "language-packs", fileName);
}
