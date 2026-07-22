using System.Runtime.InteropServices;

public interface IRemoteTokenStore
{
    string? Load(Uri endpoint);
}

public interface IRemoteTokenVault : IRemoteTokenStore
{
    void Store(Uri endpoint, string token);
    void Delete(Uri endpoint);
}

public enum RemoteTokenSource
{
    PlatformStore,
    Environment,
    LocalProcess,
}

public sealed record ResolvedRemoteToken(string Value, RemoteTokenSource Source);

public static class RemoteTokenResolver
{
    public const string EnvironmentVariable = "LESERPENT_REMOTE_TOKEN";

    public static ResolvedRemoteToken Resolve(
        Uri endpoint,
        string? environmentToken = null,
        IRemoteTokenStore? store = null)
    {
        var platformToken = (store ?? PlatformRemoteTokenStore.Instance).Load(endpoint);
        if (platformToken is not null)
        {
            RemoteClientOptions.ValidateToken(platformToken);
            return new ResolvedRemoteToken(platformToken, RemoteTokenSource.PlatformStore);
        }
        environmentToken ??= Environment.GetEnvironmentVariable(EnvironmentVariable);
        if (environmentToken is null)
        {
            throw new InvalidDataException(
                "remote token is absent from the platform credential store and LESERPENT_REMOTE_TOKEN");
        }
        RemoteClientOptions.ValidateToken(environmentToken);
        return new ResolvedRemoteToken(environmentToken, RemoteTokenSource.Environment);
    }

    public static void Store(
        Uri endpoint,
        string token,
        IRemoteTokenVault? vault = null)
    {
        RemoteClientOptions.ValidateToken(token);
        (vault ?? PlatformRemoteTokenStore.Instance).Store(endpoint, token);
    }

    public static void Delete(Uri endpoint, IRemoteTokenVault? vault = null) =>
        (vault ?? PlatformRemoteTokenStore.Instance).Delete(endpoint);

    public static string Account(Uri endpoint) => endpoint.GetComponents(
        UriComponents.SchemeAndServer,
        UriFormat.UriEscaped).ToLowerInvariant();
}

public static class BootstrapSessionCredentialResolver
{
    public const string DefaultService = "org.gewyvern.leserpent.adapters";
    private const string HandlePrefix = "vault:leserpentd:";

    public static string Resolve(string handle)
    {
        var account = ParseHandle(handle);
        string? token;
        if (OperatingSystem.IsMacOS())
        {
            token = MacKeychain.Load(DefaultService, account);
        }
        else if (OperatingSystem.IsLinux())
        {
            token = LinuxSecretService.LoadAccount(DefaultService, account);
        }
        else
        {
            throw new PlatformNotSupportedException(
                "bootstrap session promotion requires macOS Keychain or Linux Secret Service");
        }
        if (token is null)
        {
            throw new InvalidDataException(
                "bootstrap session credential is absent from the local platform store");
        }
        RemoteClientOptions.ValidateToken(token);
        return token;
    }

    public static void VerifyContract()
    {
        if (ParseHandle("vault:leserpentd:target-session") != "target-session")
        {
            throw new InvalidDataException("bootstrap session handle projection drifted");
        }
        ExpectInvalid("vault:ssh:target-session");
        ExpectInvalid("vault:leserpentd:bad/key");
    }

    private static string ParseHandle(string handle)
    {
        if (handle.Length is <= 0 or > 128
            || !handle.StartsWith(HandlePrefix, StringComparison.Ordinal))
        {
            throw new InvalidDataException("bootstrap session handle is invalid");
        }
        var key = handle[HandlePrefix.Length..];
        if (key.Length == 0 || key.Any(character =>
                !char.IsAsciiLetterOrDigit(character)
                && character is not ('.' or '_' or '-')))
        {
            throw new InvalidDataException("bootstrap session handle is invalid");
        }
        return key;
    }

    private static void ExpectInvalid(string handle)
    {
        try
        {
            _ = ParseHandle(handle);
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException("bootstrap session resolver accepted an invalid handle");
    }
}

public sealed class PlatformRemoteTokenStore : IRemoteTokenVault
{
    public const string Service = "org.gewyvern.leserpent.remote";
    public static PlatformRemoteTokenStore Instance { get; } = new();

    private PlatformRemoteTokenStore()
    {
    }

    public string? Load(Uri endpoint)
    {
        try
        {
            if (OperatingSystem.IsMacOS())
            {
                return MacKeychain.Load(Service, RemoteTokenResolver.Account(endpoint));
            }
            if (OperatingSystem.IsLinux())
            {
                return LinuxSecretService.Load(
                    Service,
                    RemoteTokenResolver.Account(endpoint));
            }
            return null;
        }
        catch (DllNotFoundException)
        {
            return null;
        }
        catch (EntryPointNotFoundException)
        {
            return null;
        }
    }

    public void Store(Uri endpoint, string token)
    {
        RemoteClientOptions.ValidateToken(token);
        var account = RemoteTokenResolver.Account(endpoint);
        if (OperatingSystem.IsMacOS())
        {
            MacKeychain.Store(Service, account, token);
            return;
        }
        if (OperatingSystem.IsLinux())
        {
            LinuxSecretService.Store(Service, account, token);
            return;
        }
        throw new PlatformNotSupportedException(
            "platform credential writes require macOS Keychain or Linux Secret Service");
    }

    public void Delete(Uri endpoint)
    {
        var account = RemoteTokenResolver.Account(endpoint);
        if (OperatingSystem.IsMacOS())
        {
            MacKeychain.Delete(Service, account);
            return;
        }
        if (OperatingSystem.IsLinux())
        {
            LinuxSecretService.Delete(Service, account);
            return;
        }
        throw new PlatformNotSupportedException(
            "platform credential deletion requires macOS Keychain or Linux Secret Service");
    }
}

internal static partial class MacKeychain
{
    private const int ItemNotFound = -25300;

    public static string? Load(string service, string account)
    {
        var status = SecKeychainFindGenericPassword(
            IntPtr.Zero,
            checked((uint)System.Text.Encoding.UTF8.GetByteCount(service)),
            service,
            checked((uint)System.Text.Encoding.UTF8.GetByteCount(account)),
            account,
            out var length,
            out var data,
            out var item);
        if (status == ItemNotFound)
        {
            return null;
        }
        if (status != 0 || data == IntPtr.Zero || item == IntPtr.Zero)
        {
            throw new InvalidOperationException(
                $"macOS Keychain rejected the remote token lookup ({status})");
        }
        try
        {
            return Marshal.PtrToStringUTF8(data, checked((int)length))
                ?? throw new InvalidDataException("macOS Keychain returned an invalid token");
        }
        finally
        {
            _ = SecKeychainItemFreeContent(IntPtr.Zero, data);
            CFRelease(item);
        }
    }

    public static void Store(string service, string account, string token)
    {
        var tokenData = Marshal.StringToCoTaskMemUTF8(token);
        try
        {
            var status = SecKeychainFindGenericPassword(
                IntPtr.Zero,
                Utf8Length(service),
                service,
                Utf8Length(account),
                account,
                out _,
                out var existingData,
                out var item);
            if (status == 0)
            {
                if (existingData != IntPtr.Zero)
                {
                    _ = SecKeychainItemFreeContent(IntPtr.Zero, existingData);
                }
                if (item == IntPtr.Zero)
                {
                    throw new InvalidOperationException("macOS Keychain returned no item reference");
                }
                try
                {
                    status = SecKeychainItemModifyAttributesAndData(
                        item,
                        IntPtr.Zero,
                        Utf8Length(token),
                        tokenData);
                }
                finally
                {
                    CFRelease(item);
                }
            }
            else if (status == ItemNotFound)
            {
                status = SecKeychainAddGenericPassword(
                    IntPtr.Zero,
                    Utf8Length(service),
                    service,
                    Utf8Length(account),
                    account,
                    Utf8Length(token),
                    tokenData,
                    out var addedItem);
                if (addedItem != IntPtr.Zero)
                {
                    CFRelease(addedItem);
                }
            }
            if (status != 0)
            {
                throw new InvalidOperationException(
                    $"macOS Keychain rejected the remote token write ({status})");
            }
        }
        finally
        {
            Marshal.ZeroFreeCoTaskMemUTF8(tokenData);
        }
    }

    public static void Delete(string service, string account)
    {
        var status = SecKeychainFindGenericPassword(
            IntPtr.Zero,
            Utf8Length(service),
            service,
            Utf8Length(account),
            account,
            out _,
            out var data,
            out var item);
        if (status == ItemNotFound)
        {
            return;
        }
        if (data != IntPtr.Zero)
        {
            _ = SecKeychainItemFreeContent(IntPtr.Zero, data);
        }
        if (status != 0 || item == IntPtr.Zero)
        {
            throw new InvalidOperationException(
                $"macOS Keychain rejected the remote token lookup ({status})");
        }
        try
        {
            status = SecKeychainItemDelete(item);
        }
        finally
        {
            CFRelease(item);
        }
        if (status != 0 && status != ItemNotFound)
        {
            throw new InvalidOperationException(
                $"macOS Keychain rejected the remote token deletion ({status})");
        }
    }

    private static uint Utf8Length(string value) => checked(
        (uint)System.Text.Encoding.UTF8.GetByteCount(value));

    [LibraryImport(
        "/System/Library/Frameworks/Security.framework/Security",
        StringMarshalling = StringMarshalling.Utf8)]
    private static partial int SecKeychainFindGenericPassword(
        IntPtr keychain,
        uint serviceLength,
        string serviceName,
        uint accountLength,
        string accountName,
        out uint passwordLength,
        out IntPtr passwordData,
        out IntPtr itemReference);

    [LibraryImport("/System/Library/Frameworks/Security.framework/Security")]
    private static partial int SecKeychainItemFreeContent(
        IntPtr attributeList,
        IntPtr data);

    [LibraryImport(
        "/System/Library/Frameworks/Security.framework/Security",
        StringMarshalling = StringMarshalling.Utf8)]
    private static partial int SecKeychainAddGenericPassword(
        IntPtr keychain,
        uint serviceLength,
        string serviceName,
        uint accountLength,
        string accountName,
        uint passwordLength,
        IntPtr passwordData,
        out IntPtr itemReference);

    [LibraryImport("/System/Library/Frameworks/Security.framework/Security")]
    private static partial int SecKeychainItemModifyAttributesAndData(
        IntPtr itemReference,
        IntPtr attributes,
        uint passwordLength,
        IntPtr passwordData);

    [LibraryImport("/System/Library/Frameworks/Security.framework/Security")]
    private static partial int SecKeychainItemDelete(IntPtr itemReference);

    [LibraryImport("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")]
    private static partial void CFRelease(IntPtr value);
}

internal static partial class LinuxSecretService
{
    private const int AttributeString = 0;
    private const int DontMatchSchemaName = 2;

    public static string? Load(string service, string endpoint) =>
        Load(service, "endpoint", endpoint);

    public static string? LoadAccount(string service, string account) =>
        Load(service, "account", account);

    private static string? Load(string service, string attribute, string value)
    {
        var schema = SecretSchemaNew(
            service,
            DontMatchSchemaName,
            "service",
            AttributeString,
            attribute,
            AttributeString,
            IntPtr.Zero);
        if (schema == IntPtr.Zero)
        {
            throw new InvalidOperationException("Linux Secret Service schema creation failed");
        }
        try
        {
            var password = SecretPasswordLookupSync(
                schema,
                IntPtr.Zero,
                out var error,
                "service",
                service,
                attribute,
                value,
                IntPtr.Zero);
            if (error != IntPtr.Zero)
            {
                GErrorFree(error);
                throw new InvalidOperationException(
                    "Linux Secret Service rejected the remote token lookup");
            }
            if (password == IntPtr.Zero)
            {
                return null;
            }
            try
            {
                return Marshal.PtrToStringUTF8(password)
                    ?? throw new InvalidDataException(
                        "Linux Secret Service returned an invalid token");
            }
            finally
            {
                SecretPasswordFree(password);
            }
        }
        finally
        {
            SecretSchemaUnref(schema);
        }
    }

    public static void Store(string service, string endpoint, string token)
    {
        var schema = CreateSchema(service);
        try
        {
            var stored = SecretPasswordStoreSync(
                schema,
                IntPtr.Zero,
                "Leserpent remote token",
                token,
                IntPtr.Zero,
                out var error,
                "service",
                service,
                "endpoint",
                endpoint,
                IntPtr.Zero);
            EnsureMutation(stored, error, "write");
        }
        finally
        {
            SecretSchemaUnref(schema);
        }
    }

    public static void Delete(string service, string endpoint)
    {
        var schema = CreateSchema(service);
        try
        {
            var cleared = SecretPasswordClearSync(
                schema,
                IntPtr.Zero,
                out var error,
                "service",
                service,
                "endpoint",
                endpoint,
                IntPtr.Zero);
            EnsureMutation(cleared, error, "deletion");
        }
        finally
        {
            SecretSchemaUnref(schema);
        }
    }

    private static IntPtr CreateSchema(string service)
    {
        var schema = SecretSchemaNew(
            service,
            DontMatchSchemaName,
            "service",
            AttributeString,
            "endpoint",
            AttributeString,
            IntPtr.Zero);
        return schema == IntPtr.Zero
            ? throw new InvalidOperationException("Linux Secret Service schema creation failed")
            : schema;
    }

    private static void EnsureMutation(int succeeded, IntPtr error, string operation)
    {
        if (error != IntPtr.Zero)
        {
            GErrorFree(error);
            throw new InvalidOperationException(
                $"Linux Secret Service rejected the token {operation}");
        }
        if (succeeded == 0)
        {
            throw new InvalidOperationException(
                $"Linux Secret Service did not complete the token {operation}");
        }
    }

    [LibraryImport("libsecret-1.so.0", EntryPoint = "secret_schema_new",
        StringMarshalling = StringMarshalling.Utf8)]
    private static partial IntPtr SecretSchemaNew(
        string name,
        int flags,
        string serviceAttribute,
        int serviceAttributeType,
        string endpointAttribute,
        int endpointAttributeType,
        IntPtr terminator);

    [LibraryImport("libsecret-1.so.0", EntryPoint = "secret_schema_unref")]
    private static partial void SecretSchemaUnref(IntPtr schema);

    [LibraryImport("libsecret-1.so.0", EntryPoint = "secret_password_lookup_sync",
        StringMarshalling = StringMarshalling.Utf8)]
    private static partial IntPtr SecretPasswordLookupSync(
        IntPtr schema,
        IntPtr cancellable,
        out IntPtr error,
        string serviceAttribute,
        string service,
        string endpointAttribute,
        string endpoint,
        IntPtr terminator);

    [LibraryImport("libsecret-1.so.0", EntryPoint = "secret_password_store_sync",
        StringMarshalling = StringMarshalling.Utf8)]
    private static partial int SecretPasswordStoreSync(
        IntPtr schema,
        IntPtr collection,
        string label,
        string password,
        IntPtr cancellable,
        out IntPtr error,
        string serviceAttribute,
        string service,
        string endpointAttribute,
        string endpoint,
        IntPtr terminator);

    [LibraryImport("libsecret-1.so.0", EntryPoint = "secret_password_clear_sync",
        StringMarshalling = StringMarshalling.Utf8)]
    private static partial int SecretPasswordClearSync(
        IntPtr schema,
        IntPtr cancellable,
        out IntPtr error,
        string serviceAttribute,
        string service,
        string endpointAttribute,
        string endpoint,
        IntPtr terminator);

    [LibraryImport("libsecret-1.so.0", EntryPoint = "secret_password_free")]
    private static partial void SecretPasswordFree(IntPtr password);

    [LibraryImport("libglib-2.0.so.0", EntryPoint = "g_error_free")]
    private static partial void GErrorFree(IntPtr error);
}
