using System.Runtime.InteropServices;

public interface IRemoteTokenStore
{
    string? Load(Uri endpoint);
}

public enum RemoteTokenSource
{
    PlatformStore,
    Environment,
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

    public static string Account(Uri endpoint) => endpoint.GetComponents(
        UriComponents.SchemeAndServer,
        UriFormat.UriEscaped).ToLowerInvariant();
}

public sealed class PlatformRemoteTokenStore : IRemoteTokenStore
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
        _ = item;
        if (status == ItemNotFound)
        {
            return null;
        }
        if (status != 0 || data == IntPtr.Zero)
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
        }
    }

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
}

internal static partial class LinuxSecretService
{
    private const int AttributeString = 0;
    private const int DontMatchSchemaName = 2;

    public static string? Load(string service, string endpoint)
    {
        var schema = SecretSchemaNew(
            service,
            DontMatchSchemaName,
            "service",
            AttributeString,
            "endpoint",
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
                "endpoint",
                endpoint,
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

    [LibraryImport("libsecret-1.so.0", EntryPoint = "secret_password_free")]
    private static partial void SecretPasswordFree(IntPtr password);

    [LibraryImport("libglib-2.0.so.0", EntryPoint = "g_error_free")]
    private static partial void GErrorFree(IntPtr error);
}
