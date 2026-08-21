using System.Globalization;

internal static class DesktopDomainCatalogContract
{
    public static IReadOnlyDictionary<string, string> Catalog(
        string prefix,
        Dictionary<string, string> values) => values.ToDictionary(
            entry => $"{prefix}{entry.Key}",
            entry => entry.Value,
            StringComparer.Ordinal);

    public static void Verify(
        string domain,
        int keyCount,
        IReadOnlyList<IReadOnlyDictionary<string, string>> catalogs,
        IReadOnlyDictionary<string, int>? formattedKeys = null)
    {
        if (catalogs.Count == 0 || catalogs[0].Count != keyCount)
        {
            throw new InvalidDataException(
                $"desktop {domain} localization key contract drifted");
        }
        var expected = catalogs[0].Keys.ToHashSet(StringComparer.Ordinal);
        foreach (var catalog in catalogs)
        {
            if (catalog.Count != keyCount
                || !catalog.Keys.ToHashSet(StringComparer.Ordinal).SetEquals(expected)
                || catalog.Any(entry => !ValidEntry(
                    entry,
                    formattedKeys?.GetValueOrDefault(entry.Key) ?? 0)))
            {
                throw new InvalidDataException(
                    $"desktop {domain} localization catalog is incomplete");
            }
            foreach (var entry in catalog)
            {
                VerifyFormat(
                    domain,
                    entry.Value,
                    formattedKeys?.GetValueOrDefault(entry.Key) ?? 0);
            }
        }
    }

    private static bool ValidEntry(KeyValuePair<string, string> entry, int arity) =>
        entry.Key.Length is > 0 and <= 128
        && entry.Value.Length is > 0 and <= 1024
        && !entry.Key.Any(char.IsControl)
        && !entry.Value.Any(char.IsControl)
        && HasExpectedPlaceholders(entry.Value, arity);

    private static bool HasExpectedPlaceholders(string value, int arity)
    {
        for (var index = 0; index < 6; index++)
        {
            if (value.Contains($"{{{index}}}", StringComparison.Ordinal) != (index < arity))
            {
                return false;
            }
        }
        return !value.Contains('{') || arity > 0;
    }

    private static void VerifyFormat(string domain, string format, int arity)
    {
        try
        {
            var values = Enumerable.Repeat<object>("fixture", arity).ToArray();
            var value = string.Format(CultureInfo.InvariantCulture, format, values);
            if (string.IsNullOrWhiteSpace(value) || value.Any(char.IsControl))
            {
                throw new InvalidDataException(
                    $"desktop {domain} localization produced invalid text");
            }
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                $"desktop {domain} localization format is invalid",
                error);
        }
    }
}
