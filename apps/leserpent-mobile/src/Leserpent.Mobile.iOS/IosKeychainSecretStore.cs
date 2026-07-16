using Foundation;
using Security;
using System.Text;

public sealed class IosKeychainSecretStore : IMobileSecretStore
{
    private const string Service = "org.gewyvern.leserpent.remote";
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private readonly object gate = new();

    public ValueTask<string?> LoadAsync(string alias, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        lock (gate)
        {
            cancellationToken.ThrowIfCancellationRequested();
            using var query = Query(alias);
            using var data = SecKeyChain.QueryAsData(query, false, out var status);
            if (status == SecStatusCode.ItemNotFound)
            {
                return ValueTask.FromResult<string?>(null);
            }
            EnsureSuccess(status, "read");
            if (data is null)
            {
                throw new InvalidDataException("iOS Keychain returned an empty credential.");
            }

            try
            {
                return ValueTask.FromResult<string?>(StrictUtf8.GetString(data.ToArray()));
            }
            catch (DecoderFallbackException)
            {
                throw new InvalidDataException("iOS Keychain credential encoding is invalid.");
            }
        }
    }

    public ValueTask StoreAsync(
        string alias,
        string secret,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        lock (gate)
        {
            cancellationToken.ThrowIfCancellationRequested();
            using var query = Query(alias);
            using var value = new SecRecord
            {
                ValueData = NSData.FromArray(StrictUtf8.GetBytes(secret)),
                Accessible = SecAccessible.WhenUnlockedThisDeviceOnly,
            };
            var status = SecKeyChain.Update(query, value);
            if (status == SecStatusCode.ItemNotFound)
            {
                using var record = Query(alias);
                record.ValueData = value.ValueData;
                record.Accessible = SecAccessible.WhenUnlockedThisDeviceOnly;
                status = SecKeyChain.Add(record);
                if (status == SecStatusCode.DuplicateItem)
                {
                    status = SecKeyChain.Update(query, value);
                }
            }
            EnsureSuccess(status, "write");
        }
        return ValueTask.CompletedTask;
    }

    public ValueTask DeleteAsync(string alias, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        lock (gate)
        {
            cancellationToken.ThrowIfCancellationRequested();
            using var query = Query(alias);
            var status = SecKeyChain.Remove(query);
            if (status != SecStatusCode.ItemNotFound)
            {
                EnsureSuccess(status, "delete");
            }
        }
        return ValueTask.CompletedTask;
    }

    private static SecRecord Query(string alias) => new(SecKind.GenericPassword)
    {
        Service = Service,
        Account = alias,
    };

    private static void EnsureSuccess(SecStatusCode status, string operation)
    {
        if (status != SecStatusCode.Success)
        {
            throw new InvalidOperationException($"iOS Keychain {operation} failed ({status}).");
        }
    }
}
