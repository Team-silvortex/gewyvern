#if DEBUG
using Foundation;

internal static class IosPlatformProof
{
    private const string FileName = "leserpent-ios-keychain-proof.json";

    public static async Task RunKeychainAsync()
    {
        var path = Path.Combine(CacheDirectory(), FileName);
        var temporary = $"{path}.{Guid.NewGuid():N}.tmp";
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
            var endpoint = new Uri("https://ios-keychain-proof.invalid:9443/");
            var token = new string('k', 32);
            var vault = new MobileCredentialVault(new IosKeychainSecretStore());
            var alias = MobileCredentialVault.Alias(endpoint);
            await vault.StoreAsync(endpoint, token, CancellationToken.None);
            var loaded = await vault.LoadAsync(endpoint, CancellationToken.None);
            await vault.DeleteAsync(endpoint, CancellationToken.None);
            var deleted = await vault.LoadAsync(endpoint, CancellationToken.None) is null;
            var passed = loaded == token
                && deleted
                && !alias.Contains("ios-keychain-proof", StringComparison.Ordinal);
            WriteResult(temporary, path, passed
                ? "{\"schema_version\":1,\"keychain_round_trip\":true,\"endpoint_opaque_alias\":true,\"delete_verified\":true,\"sensitive_values_retained\":false}"
                : "{\"schema_version\":1,\"keychain_round_trip\":false,\"endpoint_opaque_alias\":false,\"delete_verified\":false,\"sensitive_values_retained\":false}");
        }
        catch
        {
            WriteResult(temporary, path,
                "{\"schema_version\":1,\"keychain_round_trip\":false,\"endpoint_opaque_alias\":false,\"delete_verified\":false,\"sensitive_values_retained\":false}");
        }
        finally
        {
            if (File.Exists(temporary))
            {
                File.Delete(temporary);
            }
        }
    }

    private static string CacheDirectory() =>
        NSSearchPath.GetDirectories(
                NSSearchPathDirectory.CachesDirectory,
                NSSearchPathDomain.User,
                true)
            .SingleOrDefault()
        ?? throw new InvalidOperationException("iOS cache storage is unavailable.");

    private static void WriteResult(string temporary, string path, string value)
    {
        File.WriteAllText(temporary, value);
        File.Move(temporary, path, true);
    }
}
#endif
