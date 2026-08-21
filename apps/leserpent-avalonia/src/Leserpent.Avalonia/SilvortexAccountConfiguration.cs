using System.Text;
using System.Xml;
using System.Xml.Linq;

internal enum SilvortexAccountConfigurationSource
{
    Disabled,
    PackagedBundle,
    Environment,
}

internal sealed record SilvortexAccountConfiguration(
    SilvortexAccountOptions? Options,
    string Message,
    SilvortexAccountConfigurationSource Source,
    SilvortexAccountStatus Status = SilvortexAccountStatus.Raw,
    string? StatusDetail = null);

internal static class SilvortexAccountConfigurationLoader
{
    internal const string PackagedIssuerKey = "LeserpentSilvortexIssuer";
    private const int MaxPlistBytes = 64 * 1024;

    public static SilvortexAccountConfiguration Load()
    {
        var environmentIssuer = Environment.GetEnvironmentVariable(
            SilvortexAccountOptions.IssuerEnvironmentVariable);
        var environmentClientId = Environment.GetEnvironmentVariable(
            SilvortexAccountOptions.ClientIdEnvironmentVariable);
        var environmentPort = Environment.GetEnvironmentVariable(
            SilvortexAccountOptions.CallbackPortEnvironmentVariable);
        var environmentAllowInsecure = Environment.GetEnvironmentVariable(
            SilvortexAccountOptions.AllowInsecureEnvironmentVariable);
        var plistPath = ResolvePackagedInfoPlist(
            Environment.ProcessPath,
            OperatingSystem.IsMacOS());
        if (plistPath is null)
        {
            return Resolve(
                packagedBundle: false,
                packagedIssuer: null,
                environmentIssuer,
                environmentClientId,
                environmentPort,
                environmentAllowInsecure);
        }

        string? packagedIssuer;
        try
        {
            packagedIssuer = ReadPackagedIssuer(plistPath);
        }
        catch (Exception error) when (error is IOException
            or UnauthorizedAccessException
            or InvalidDataException
            or XmlException)
        {
            return new SilvortexAccountConfiguration(
                null,
                "The packaged Team Silvortex configuration is invalid.",
                SilvortexAccountConfigurationSource.PackagedBundle,
                SilvortexAccountStatus.ConfigurationInvalid,
                "The packaged Team Silvortex configuration is invalid.");
        }
        return Resolve(
            packagedBundle: true,
            packagedIssuer,
            environmentIssuer,
            environmentClientId,
            environmentPort,
            environmentAllowInsecure);
    }

    public static void VerifyContract()
    {
        var issuer = "https://id.example.invalid/";
        var plist = $"""
            <?xml version="1.0" encoding="UTF-8"?>
            <plist version="1.0"><dict>
              <key>CFBundleIdentifier</key><string>org.gewyvern.leserpent</string>
              <key>CFBundleExecutable</key><string>Leserpent.Avalonia</string>
              <key>CFBundlePackageType</key><string>APPL</string>
              <key>{PackagedIssuerKey}</key><string>{issuer}</string>
            </dict></plist>
            """;
        if (ParsePackagedIssuer(Encoding.UTF8.GetBytes(plist)) != issuer)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex issuer projection drifted.");
        }
        var packaged = Resolve(
            packagedBundle: true,
            packagedIssuer: issuer,
            environmentIssuer: null,
            environmentClientId: null,
            environmentPort: null,
            environmentAllowInsecure: null);
        if (packaged.Source != SilvortexAccountConfigurationSource.PackagedBundle
            || packaged.Options?.ClientId != SilvortexAccountOptions.ReviewedClientId
            || packaged.Options.CallbackPort != SilvortexAccountOptions.DefaultCallbackPort)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex reviewed-client configuration drifted.");
        }
        var overridden = Resolve(
            packagedBundle: true,
            packagedIssuer: issuer,
            environmentIssuer: "https://attacker.example.invalid/",
            environmentClientId: null,
            environmentPort: null,
            environmentAllowInsecure: null);
        if (overridden.Options is not null
            || overridden.Source != SilvortexAccountConfigurationSource.PackagedBundle)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex configuration accepted an environment override.");
        }
        var development = Resolve(
            packagedBundle: false,
            packagedIssuer: null,
            environmentIssuer: issuer,
            environmentClientId: null,
            environmentPort: null,
            environmentAllowInsecure: null);
        if (development.Source != SilvortexAccountConfigurationSource.Environment
            || development.Options?.Issuer.AbsoluteUri != issuer)
        {
            throw new InvalidDataException(
                "Team Silvortex development environment configuration drifted.");
        }
        var syntheticExecutable = Path.Combine(
            Path.DirectorySeparatorChar.ToString(),
            "Applications",
            "Leserpent.app",
            "Contents",
            "MacOS",
            "Leserpent.Avalonia");
        if (ResolvePackagedInfoPlist(syntheticExecutable, isMacOS: true)
                != Path.Combine(
                    Path.DirectorySeparatorChar.ToString(),
                    "Applications",
                    "Leserpent.app",
                    "Contents",
                    "Info.plist")
            || ResolvePackagedInfoPlist(syntheticExecutable, isMacOS: false) is not null
            || ResolvePackagedInfoPlist("/tmp/Leserpent.Avalonia", isMacOS: true) is not null)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex application boundary drifted.");
        }

        ExpectInvalidPlist(plist.Replace(
            $"<key>{PackagedIssuerKey}</key>",
            $"<key>{PackagedIssuerKey}</key><string>{issuer}</string>"
                + $"<key>{PackagedIssuerKey}</key>"));
        ExpectInvalidPlist(plist.Replace(
            $"<string>{issuer}</string>",
            "<true/>"));
        ExpectInvalidPlist($"""
            <?xml version="1.0"?>
            <!DOCTYPE plist [<!ENTITY issuer "https://id.example.invalid/">]>
            <plist version="1.0"><dict>
              <key>CFBundleIdentifier</key><string>org.gewyvern.leserpent</string>
              <key>CFBundleExecutable</key><string>Leserpent.Avalonia</string>
              <key>CFBundlePackageType</key><string>APPL</string>
              <key>{PackagedIssuerKey}</key><string>&issuer;</string>
            </dict></plist>
            """);
    }

    private static SilvortexAccountConfiguration Resolve(
        bool packagedBundle,
        string? packagedIssuer,
        string? environmentIssuer,
        string? environmentClientId,
        string? environmentPort,
        string? environmentAllowInsecure)
    {
        environmentIssuer = environmentIssuer?.Trim();
        environmentClientId = environmentClientId?.Trim();
        environmentPort = environmentPort?.Trim();
        environmentAllowInsecure = environmentAllowInsecure?.Trim();
        if (packagedBundle)
        {
            if (HasValue(environmentIssuer)
                || HasValue(environmentClientId)
                || HasValue(environmentPort)
                || HasValue(environmentAllowInsecure))
            {
                return new SilvortexAccountConfiguration(
                    null,
                    "Packaged Team Silvortex configuration refuses environment overrides.",
                    SilvortexAccountConfigurationSource.PackagedBundle,
                    SilvortexAccountStatus.OverrideRefused);
            }
            if (string.IsNullOrEmpty(packagedIssuer))
            {
                return new SilvortexAccountConfiguration(
                    null,
                    "Team Silvortex sign-in is optional and is not configured in this application bundle.",
                    SilvortexAccountConfigurationSource.PackagedBundle,
                    SilvortexAccountStatus.OptionalBundle);
            }
            try
            {
                return new SilvortexAccountConfiguration(
                    SilvortexAccountOptions.Create(
                        packagedIssuer,
                        SilvortexAccountOptions.ReviewedClientId,
                        SilvortexAccountOptions.DefaultCallbackPort),
                    "Team Silvortex sign-in is ready from the application bundle.",
                    SilvortexAccountConfigurationSource.PackagedBundle,
                    SilvortexAccountStatus.BundleReady);
            }
            catch (InvalidDataException error)
            {
                return new SilvortexAccountConfiguration(
                    null,
                    error.Message,
                    SilvortexAccountConfigurationSource.PackagedBundle,
                    SilvortexAccountStatus.ConfigurationInvalid,
                    error.Message);
            }
        }

        if (!HasValue(environmentIssuer) && !HasValue(environmentClientId))
        {
            return new SilvortexAccountConfiguration(
                null,
                "Team Silvortex sign-in is optional and is not configured for this build.",
                SilvortexAccountConfigurationSource.Disabled,
                SilvortexAccountStatus.OptionalBuild);
        }
        if (!HasValue(environmentIssuer))
        {
            return new SilvortexAccountConfiguration(
                null,
                $"Set {SilvortexAccountOptions.IssuerEnvironmentVariable} when configuring Team Silvortex sign-in.",
                SilvortexAccountConfigurationSource.Environment,
                SilvortexAccountStatus.MissingIssuer);
        }
        var clientId = SilvortexAccountOptions.ResolveClientId(environmentClientId);
        var allowInsecure = string.Equals(
            environmentAllowInsecure,
            "true",
            StringComparison.OrdinalIgnoreCase);
        var port = !HasValue(environmentPort)
            ? SilvortexAccountOptions.DefaultCallbackPort
            : int.TryParse(environmentPort, out var parsed) ? parsed : -1;
        try
        {
            return new SilvortexAccountConfiguration(
                SilvortexAccountOptions.Create(
                    environmentIssuer!,
                    clientId,
                    port,
                    allowInsecure),
                "Team Silvortex sign-in is ready from development configuration.",
                SilvortexAccountConfigurationSource.Environment,
                SilvortexAccountStatus.DevelopmentReady);
        }
        catch (InvalidDataException error)
        {
            return new SilvortexAccountConfiguration(
                null,
                error.Message,
                SilvortexAccountConfigurationSource.Environment,
                SilvortexAccountStatus.ConfigurationInvalid,
                error.Message);
        }
    }

    private static string? ResolvePackagedInfoPlist(string? processPath, bool isMacOS)
    {
        if (!isMacOS || string.IsNullOrEmpty(processPath))
        {
            return null;
        }
        var executable = new FileInfo(Path.GetFullPath(processPath));
        var macos = executable.Directory;
        var contents = macos?.Parent;
        var application = contents?.Parent;
        if (macos?.Name != "MacOS"
            || contents?.Name != "Contents"
            || !string.Equals(
                application?.Extension,
                ".app",
                StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }
        return Path.Combine(contents.FullName, "Info.plist");
    }

    private static string? ReadPackagedIssuer(string path)
    {
        var info = new FileInfo(path);
        if (!info.Exists
            || info.Length is <= 0 or > MaxPlistBytes
            || (info.Attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex Info.plist is unavailable or unsafe.");
        }
        using var stream = new FileStream(
            path,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read,
            bufferSize: 4096,
            FileOptions.SequentialScan);
        if (stream.Length is <= 0 or > MaxPlistBytes)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex Info.plist exceeds its size limit.");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        if (stream.ReadByte() != -1)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex Info.plist changed while it was read.");
        }
        return ParsePackagedIssuer(payload);
    }

    private static string? ParsePackagedIssuer(ReadOnlySpan<byte> payload)
    {
        if (payload.Length is <= 0 or > MaxPlistBytes)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex Info.plist exceeds its size limit.");
        }
        using var stream = new MemoryStream(payload.ToArray(), writable: false);
        using var reader = XmlReader.Create(stream, new XmlReaderSettings
        {
            DtdProcessing = DtdProcessing.Ignore,
            XmlResolver = null,
            MaxCharactersInDocument = MaxPlistBytes,
            IgnoreComments = true,
            IgnoreWhitespace = true,
        });
        var document = XDocument.Load(reader, LoadOptions.None);
        var root = document.Root;
        if (root is null
            || root.Name.LocalName != "plist"
            || root.Name.NamespaceName.Length != 0)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex Info.plist has an invalid root.");
        }
        var dictionaries = root.Elements().ToArray();
        if (dictionaries.Length != 1
            || dictionaries[0].Name.LocalName != "dict"
            || dictionaries[0].Name.NamespaceName.Length != 0)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex Info.plist has an invalid dictionary.");
        }
        var entries = dictionaries[0].Elements().ToArray();
        if (entries.Length % 2 != 0)
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex Info.plist has an incomplete entry.");
        }
        string? issuer = null;
        string? bundleIdentifier = null;
        string? bundleExecutable = null;
        string? bundlePackageType = null;
        for (var index = 0; index < entries.Length; index += 2)
        {
            var key = entries[index];
            var value = entries[index + 1];
            if (key.Name.LocalName != "key"
                || key.Name.NamespaceName.Length != 0
                || key.HasElements)
            {
                throw new InvalidDataException(
                    "Packaged Team Silvortex Info.plist has an invalid key.");
            }
            switch (key.Value)
            {
                case "CFBundleIdentifier":
                    AssignUniqueString(value, key.Value, ref bundleIdentifier);
                    break;
                case "CFBundleExecutable":
                    AssignUniqueString(value, key.Value, ref bundleExecutable);
                    break;
                case "CFBundlePackageType":
                    AssignUniqueString(value, key.Value, ref bundlePackageType);
                    break;
                case PackagedIssuerKey:
                    AssignUniqueString(
                        value,
                        key.Value,
                        ref issuer,
                        SilvortexAccountOptions.MaxIssuerLength);
                    break;
            }
        }
        if (bundleIdentifier != "org.gewyvern.leserpent"
            || bundleExecutable != "Leserpent.Avalonia"
            || bundlePackageType != "APPL")
        {
            throw new InvalidDataException(
                "Packaged Team Silvortex configuration is not bound to the Leserpent application identity.");
        }
        return issuer;
    }

    private static void AssignUniqueString(
        XElement value,
        string key,
        ref string? destination,
        int maxLength = 256)
    {
        if (destination is not null
            || value.Name.LocalName != "string"
            || value.Name.NamespaceName.Length != 0
            || value.HasElements
            || value.Value.Length is <= 0
            || value.Value.Length > maxLength)
        {
            throw new InvalidDataException(
                $"Packaged Team Silvortex Info.plist {key} is duplicate or invalid.");
        }
        destination = value.Value;
    }

    private static void ExpectInvalidPlist(string plist)
    {
        try
        {
            _ = ParsePackagedIssuer(Encoding.UTF8.GetBytes(plist));
        }
        catch (Exception error) when (error is InvalidDataException or XmlException)
        {
            return;
        }
        throw new InvalidDataException(
            "Packaged Team Silvortex parser accepted an unsafe plist.");
    }

    private static bool HasValue(string? value) => !string.IsNullOrEmpty(value);
}
