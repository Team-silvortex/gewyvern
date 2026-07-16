using Android.Content;
using Android.Security.Keystore;
using Java.Security;
using Javax.Crypto;
using Javax.Crypto.Spec;
using System.Text;

public sealed class AndroidKeystoreSecretStore(Context context) : IMobileSecretStore
{
    private const string AndroidKeyStore = "AndroidKeyStore";
    private const string MasterKeyAlias = "leserpent.remote.master.v1";
    private const string PreferencesName = "leserpent.remote.secrets.v1";
    private const string Transformation = "AES/GCM/NoPadding";
    private const int AuthenticationTagBits = 128;

    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private static readonly object Gate = new();
    private readonly ISharedPreferences preferences =
        context.ApplicationContext?.GetSharedPreferences(PreferencesName, FileCreationMode.Private)
        ?? throw new InvalidOperationException("Android application context is unavailable.");

    public ValueTask<string?> LoadAsync(string alias, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        lock (Gate)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var envelope = preferences.GetString(alias, null);
            return ValueTask.FromResult(envelope is null ? null : Decrypt(envelope));
        }
    }

    public ValueTask StoreAsync(
        string alias,
        string secret,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        lock (Gate)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var envelope = Encrypt(secret);
            using var editor = preferences.Edit()
                ?? throw new InvalidOperationException("Android secret storage is unavailable.");
            if (!editor.PutString(alias, envelope)!.Commit())
            {
                throw new InvalidOperationException("Android secret storage rejected the write.");
            }
        }
        return ValueTask.CompletedTask;
    }

    public ValueTask DeleteAsync(string alias, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        lock (Gate)
        {
            cancellationToken.ThrowIfCancellationRequested();
            using var editor = preferences.Edit()
                ?? throw new InvalidOperationException("Android secret storage is unavailable.");
            if (!editor.Remove(alias)!.Commit())
            {
                throw new InvalidOperationException("Android secret storage rejected the deletion.");
            }
        }
        return ValueTask.CompletedTask;
    }

    private static string Encrypt(string secret)
    {
        using var cipher = Cipher.GetInstance(Transformation)
            ?? throw new InvalidOperationException("Android AES-GCM is unavailable.");
        cipher.Init(CipherMode.EncryptMode, GetOrCreateMasterKey());
        var ciphertext = cipher.DoFinal(StrictUtf8.GetBytes(secret));
        var nonce = cipher.GetIV();
        if (ciphertext is null || nonce is null)
        {
            throw new InvalidOperationException("Android AES-GCM produced no encrypted value.");
        }
        return $"1:{Convert.ToBase64String(nonce)}:{Convert.ToBase64String(ciphertext)}";
    }

    private static string Decrypt(string envelope)
    {
        try
        {
            var fields = envelope.Split(':');
            if (fields is not ["1", _, _])
            {
                throw new FormatException();
            }

            var nonce = Convert.FromBase64String(fields[1]);
            var ciphertext = Convert.FromBase64String(fields[2]);
            using var parameters = new GCMParameterSpec(AuthenticationTagBits, nonce);
            using var cipher = Cipher.GetInstance(Transformation)
                ?? throw new InvalidOperationException();
            cipher.Init(CipherMode.DecryptMode, GetOrCreateMasterKey(), parameters);
            var plaintext = cipher.DoFinal(ciphertext)
                ?? throw new InvalidOperationException();
            return StrictUtf8.GetString(plaintext);
        }
        catch (Exception exception) when (
            exception is FormatException
            or DecoderFallbackException
            or GeneralSecurityException
            or Java.Lang.IllegalArgumentException
            or InvalidOperationException)
        {
            throw new InvalidDataException("Android credential storage is corrupt or unavailable.");
        }
    }

    private static Java.Security.IKey GetOrCreateMasterKey()
    {
        using var keyStore = KeyStore.GetInstance(AndroidKeyStore)
            ?? throw new InvalidOperationException("Android Keystore is unavailable.");
        keyStore.Load(null);
        if (!keyStore.ContainsAlias(MasterKeyAlias))
        {
            using var generator = KeyGenerator.GetInstance(
                KeyProperties.KeyAlgorithmAes,
                AndroidKeyStore)
                ?? throw new InvalidOperationException("Android AES key generation is unavailable.");
            using var builder = new KeyGenParameterSpec.Builder(
                MasterKeyAlias,
                KeyStorePurpose.Encrypt | KeyStorePurpose.Decrypt)
                .SetBlockModes(KeyProperties.BlockModeGcm)
                .SetEncryptionPaddings(KeyProperties.EncryptionPaddingNone)
                .SetKeySize(256);
            using var specification = builder.Build();
            generator.Init(specification);
            generator.GenerateKey()?.Dispose();
        }

        return keyStore.GetKey(MasterKeyAlias, null)
            ?? throw new InvalidOperationException("Android Keystore master key is unavailable.");
    }
}
