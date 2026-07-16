using System.Security.Cryptography;
using System.Text;

public interface IMobileSecretStore
{
    ValueTask<string?> LoadAsync(string alias, CancellationToken cancellationToken);
    ValueTask StoreAsync(string alias, string secret, CancellationToken cancellationToken);
    ValueTask DeleteAsync(string alias, CancellationToken cancellationToken);
}

public sealed class MobileCredentialVault(IMobileSecretStore store) : IMobileCredentialVault
{
    public async ValueTask<string?> LoadAsync(
        Uri endpoint,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var token = await store.LoadAsync(Alias(endpoint), cancellationToken)
            .ConfigureAwait(false);
        if (token is not null)
        {
            RemoteClientOptions.ValidateToken(token);
        }
        return token;
    }

    public async ValueTask StoreAsync(
        Uri endpoint,
        string token,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        RemoteClientOptions.ValidateToken(token);
        await store.StoreAsync(Alias(endpoint), token, cancellationToken)
            .ConfigureAwait(false);
    }

    public ValueTask DeleteAsync(Uri endpoint, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return store.DeleteAsync(Alias(endpoint), cancellationToken);
    }

    public static string Alias(Uri endpoint)
    {
        var canonical = CanonicalEndpoint(endpoint);
        var digest = SHA256.HashData(Encoding.UTF8.GetBytes(canonical));
        return $"leserpent.remote.{Convert.ToHexString(digest).ToLowerInvariant()}";
    }

    private static string CanonicalEndpoint(Uri endpoint)
    {
        var validated = RemoteClientOptions.ParseEndpoint(endpoint.AbsoluteUri);
        return RemoteTokenResolver.Account(validated);
    }
}
