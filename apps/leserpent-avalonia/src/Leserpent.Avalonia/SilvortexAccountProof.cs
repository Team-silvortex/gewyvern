using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

internal sealed record SilvortexAccountProofResult(
    string EvidencePath,
    long DurationMilliseconds);

internal static class SilvortexAccountProof
{
    public const string ContractVersion = "1.99.0";
    private const string ProofId = "leserpent-silvortex-native-desktop-account";
    private const int MaxEvidenceBytes = 16 * 1024;

    public static async Task<SilvortexAccountProofResult> RunAsync(string outputPath)
    {
        var evidencePath = ValidateOutputPath(outputPath);
        var configuration = RequireReviewedConfiguration();
        var options = configuration.Options!;
        RequireReleaseRuntime();
        EnsureFreshCredential(
            SilvortexAccountSession.HasStoredRefreshToken(options));

        byte[]? loginCredentialDigest = null;
        byte[]? restoredCredentialDigest = null;
        SilvortexAccountSession? activeSession = null;
        var completed = false;
        var timer = Stopwatch.StartNew();
        try
        {
            activeSession = SilvortexAccountSession.CreateForProof(options);
            await activeSession.SignInAsync();
            RequirePhase(activeSession, SilvortexAccountPhase.SignedIn, "system-browser sign-in");
            if (!activeSession.SystemBrowserLaunched)
            {
                throw new InvalidDataException(
                    "Team Silvortex proof did not observe a successful system-browser launch.");
            }
            if (!SilvortexAccountSession.HasStoredRefreshToken(options))
            {
                throw new InvalidDataException(
                    "Team Silvortex sign-in did not persist a platform-vault refresh credential.");
            }
            loginCredentialDigest = SilvortexAccountSession.StoredRefreshTokenDigest(options);

            activeSession.Dispose();
            activeSession = SilvortexAccountSession.CreateForProof(options);
            await activeSession.RestoreForProofAsync();
            RequirePhase(activeSession, SilvortexAccountPhase.SignedIn, "platform-vault restore");
            restoredCredentialDigest = SilvortexAccountSession.StoredRefreshTokenDigest(options);
            if (CryptographicOperations.FixedTimeEquals(
                    loginCredentialDigest,
                    restoredCredentialDigest))
            {
                throw new InvalidDataException(
                    "Team Silvortex restore did not rotate the refresh credential.");
            }

            await activeSession.SignOutAsync();
            RequirePhase(activeSession, SilvortexAccountPhase.SignedOut, "local sign-out");
            if (!activeSession.AccessTokenRevocationAttempted
                || !activeSession.RefreshTokenRevocationAttempted)
            {
                throw new InvalidDataException(
                    "Team Silvortex sign-out did not attempt both credential revocations.");
            }
            if (SilvortexAccountSession.HasStoredRefreshToken(options))
            {
                throw new InvalidDataException(
                    "Team Silvortex local sign-out retained the platform-vault credential.");
            }

            timer.Stop();
            var facts = new ProofFacts(
                OperatingSystem.IsMacOS() ? "macos" : "linux",
                RuntimeInformation.ProcessArchitecture.ToString().ToLowerInvariant(),
                ConfigurationSource(configuration.Source),
                HashCurrentBinary(),
                timer.ElapsedMilliseconds,
                SystemBrowserLaunched: true,
                LoopbackCallbackAccepted: true,
                PlatformVaultLoginPersisted: true,
                FreshSessionRestore: true,
                RefreshCredentialRotated: true,
                AccessCredentialRevocationAttempted: true,
                RefreshCredentialRevocationAttempted: true,
                LocalLogoutCompleted: true,
                PlatformVaultEmptyAfterLogout: true);
            WriteEvidence(evidencePath, facts, DateTimeOffset.UtcNow);
            completed = true;
            return new SilvortexAccountProofResult(evidencePath, timer.ElapsedMilliseconds);
        }
        finally
        {
            if (loginCredentialDigest is not null)
            {
                CryptographicOperations.ZeroMemory(loginCredentialDigest);
            }
            if (restoredCredentialDigest is not null)
            {
                CryptographicOperations.ZeroMemory(restoredCredentialDigest);
            }
            if (!completed
                && activeSession is not null
                && SilvortexAccountSession.HasStoredRefreshToken(options))
            {
                await activeSession.SignOutAsync();
            }
            activeSession?.Dispose();
            if (!completed && SilvortexAccountSession.HasStoredRefreshToken(options))
            {
                SilvortexAccountSession.DeleteStoredRefreshToken(options);
            }
        }
    }

    public static void VerifyContract()
    {
        var options = SilvortexAccountOptions.Create(
            "https://id.example.invalid/",
            SilvortexAccountOptions.ReviewedClientId,
            SilvortexAccountOptions.DefaultCallbackPort);
        EnsureReviewedConfiguration(options);
        EnsureReleaseConfiguration(
            new SilvortexAccountConfiguration(
                options,
                "fixture",
                SilvortexAccountConfigurationSource.PackagedBundle),
            requirePackagedBundle: true);
        EnsureReleaseConfiguration(
            new SilvortexAccountConfiguration(
                options,
                "fixture",
                SilvortexAccountConfigurationSource.Environment),
            requirePackagedBundle: false);
        ExpectRejected(() => EnsureReleaseConfiguration(
            new SilvortexAccountConfiguration(
                options,
                "fixture",
                SilvortexAccountConfigurationSource.Environment),
            requirePackagedBundle: true));
        ExpectRejected(() => EnsureReviewedConfiguration(options with
        {
            ClientId = "svx_client_self_hosted_fixture",
        }));
        ExpectRejected(() => EnsureReviewedConfiguration(options with
        {
            CallbackPort = SilvortexAccountOptions.DefaultCallbackPort + 1,
        }));
        ExpectRejected(() => EnsureFreshCredential(credentialExists: true));
        ExpectRejected(() => ValidateOutputPath("relative-proof.json"));

        var root = Directory.CreateTempSubdirectory("leserpent-account-proof-");
        try
        {
            var facts = new ProofFacts(
                "macos",
                "arm64",
                "packaged-info-plist",
                new string('a', 64),
                DurationMilliseconds: 1,
                SystemBrowserLaunched: true,
                LoopbackCallbackAccepted: true,
                PlatformVaultLoginPersisted: true,
                FreshSessionRestore: true,
                RefreshCredentialRotated: true,
                AccessCredentialRevocationAttempted: true,
                RefreshCredentialRevocationAttempted: true,
                LocalLogoutCompleted: true,
                PlatformVaultEmptyAfterLogout: true);
            var evidencePath = Path.Combine(root.FullName, "proof.json");
            WriteEvidence(evidencePath, facts, DateTimeOffset.UnixEpoch);
            using var evidence = JsonDocument.Parse(File.ReadAllBytes(evidencePath));
            var document = evidence.RootElement;
            if (document.GetProperty("schema_version").GetInt32() != 1
                || document.GetProperty("proof").GetString() != ProofId
                || document.GetProperty("source").GetProperty("avalonia_contract").GetString()
                    != ContractVersion
                || document.GetProperty("target").GetProperty("configuration_source").GetString()
                    != "packaged-info-plist"
                || document.GetProperty("observations")
                    .GetProperty("platform_vault_empty_after_logout").ValueKind
                    != JsonValueKind.True
                || document.GetProperty("boundaries")
                    .GetProperty("account_identity_written").ValueKind
                    != JsonValueKind.False
                || document.GetProperty("boundaries")
                    .GetProperty("environment_override_accepted").ValueKind
                    != JsonValueKind.False
                || document.GetProperty("result").GetString() != "passed")
            {
                throw new InvalidDataException(
                    "Team Silvortex desktop proof evidence contract drifted.");
            }
            if (new FileInfo(evidencePath).Length is <= 0 or > MaxEvidenceBytes)
            {
                throw new InvalidDataException(
                    "Team Silvortex desktop proof evidence is not bounded.");
            }
            if (!OperatingSystem.IsWindows()
                && File.GetUnixFileMode(evidencePath)
                    != (UnixFileMode.UserRead | UnixFileMode.UserWrite))
            {
                throw new InvalidDataException(
                    "Team Silvortex desktop proof evidence is not private.");
            }
            var serialized = File.ReadAllText(evidencePath, Encoding.UTF8);
            foreach (var forbidden in new[]
            {
                "id.example.invalid",
                "fixture@example.invalid",
                "Bearer ",
                "refresh-credential-value",
                "account-subject-value",
            })
            {
                if (serialized.Contains(forbidden, StringComparison.OrdinalIgnoreCase))
                {
                    throw new InvalidDataException(
                        "Team Silvortex desktop proof evidence retained sensitive identity material.");
                }
            }
            ExpectRejected(() => WriteEvidence(evidencePath, facts, DateTimeOffset.UnixEpoch));
            ExpectRejected(() => WriteEvidence(
                Path.Combine(root.FullName, "invalid-facts.json"),
                facts with { SystemBrowserLaunched = false },
                DateTimeOffset.UnixEpoch));

            if (!OperatingSystem.IsWindows())
            {
                var actualDirectory = Directory.CreateDirectory(
                    Path.Combine(root.FullName, "actual"));
                var linkedDirectory = Path.Combine(root.FullName, "linked");
                Directory.CreateSymbolicLink(linkedDirectory, actualDirectory.FullName);
                ExpectRejected(() => WriteEvidence(
                    Path.Combine(linkedDirectory, "proof.json"),
                    facts,
                    DateTimeOffset.UnixEpoch));
            }
            if (Directory.EnumerateFiles(
                    root.FullName,
                    "*.tmp",
                    SearchOption.AllDirectories).Any())
            {
                throw new InvalidDataException(
                    "Team Silvortex desktop proof left a temporary evidence file.");
            }
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }

    private static SilvortexAccountConfiguration RequireReviewedConfiguration()
    {
        var configuration = SilvortexAccountConfigurationLoader.Load();
        var options = configuration.Options
            ?? throw new InvalidDataException(configuration.Message);
        EnsureReviewedConfiguration(options);
        EnsureReleaseConfiguration(
            configuration,
            requirePackagedBundle: OperatingSystem.IsMacOS());
        return configuration;
    }

    private static void EnsureReleaseConfiguration(
        SilvortexAccountConfiguration configuration,
        bool requirePackagedBundle)
    {
        var expected = requirePackagedBundle
            ? SilvortexAccountConfigurationSource.PackagedBundle
            : SilvortexAccountConfigurationSource.Environment;
        if (configuration.Source != expected)
        {
            throw new InvalidDataException(requirePackagedBundle
                ? "macOS desktop account proof requires the reviewed issuer embedded in the application bundle."
                : "Linux desktop account proof requires an explicit HTTPS issuer environment configuration.");
        }
    }

    private static string ConfigurationSource(SilvortexAccountConfigurationSource source) =>
        source switch
        {
            SilvortexAccountConfigurationSource.PackagedBundle => "packaged-info-plist",
            SilvortexAccountConfigurationSource.Environment => "environment",
            _ => throw new InvalidDataException(
                "Desktop account proof has no active Team Silvortex configuration source."),
        };

    private static void EnsureReviewedConfiguration(SilvortexAccountOptions options)
    {
        if (options.ClientId != SilvortexAccountOptions.ReviewedClientId
            || options.CallbackPort != SilvortexAccountOptions.DefaultCallbackPort
            || options.RedirectUri.AbsoluteUri
                != "http://127.0.0.1:43817/oidc/callback"
            || options.Issuer.Scheme != Uri.UriSchemeHttps)
        {
            throw new InvalidDataException(
                "Desktop account proof requires the reviewed client, fixed callback, and an HTTPS issuer.");
        }
    }

    private static void RequireReleaseRuntime()
    {
        if (!OperatingSystem.IsMacOS() && !OperatingSystem.IsLinux())
        {
            throw new PlatformNotSupportedException(
                "Desktop account proof requires macOS Keychain or Linux Secret Service.");
        }
        if (RuntimeFeature.IsDynamicCodeSupported || Environment.ProcessPath is null)
        {
            throw new InvalidDataException(
                "Desktop account proof must run from the packaged NativeAOT executable.");
        }
    }

    private static void EnsureFreshCredential(bool credentialExists)
    {
        if (credentialExists)
        {
            throw new InvalidDataException(
                "Desktop account proof refuses to replace an existing Team Silvortex credential; sign out normally before running it.");
        }
    }

    private static void RequirePhase(
        SilvortexAccountSession session,
        SilvortexAccountPhase expected,
        string operation)
    {
        if (session.Snapshot.Phase != expected)
        {
            throw new InvalidDataException(
                $"Team Silvortex {operation} did not reach {expected}: {session.Snapshot.Message}");
        }
    }

    private static string HashCurrentBinary()
    {
        var processPath = Environment.ProcessPath
            ?? throw new InvalidDataException("Desktop proof executable path is unavailable.");
        using var stream = File.OpenRead(processPath);
        return Convert.ToHexString(SHA256.HashData(stream)).ToLowerInvariant();
    }

    private static string ValidateOutputPath(string path)
    {
        if (string.IsNullOrWhiteSpace(path)
            || path.Length > 4096
            || path.Any(char.IsControl)
            || !Path.IsPathFullyQualified(path)
            || !string.Equals(Path.GetExtension(path), ".json", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException(
                "Desktop account proof output must be an absolute JSON path.");
        }
        var fullPath = Path.GetFullPath(path);
        if (!string.Equals(path, fullPath, StringComparison.Ordinal)
            || File.Exists(fullPath)
            || Directory.Exists(fullPath))
        {
            throw new InvalidDataException(
                "Desktop account proof output must be canonical and must not already exist.");
        }
        var directory = Path.GetDirectoryName(fullPath)
            ?? throw new InvalidDataException(
                "Desktop account proof output directory is unavailable.");
        if (!Directory.Exists(directory)
            || (File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                "Desktop account proof output directory must exist and must not be a symbolic link.");
        }
        return fullPath;
    }

    private static void WriteEvidence(
        string path,
        ProofFacts facts,
        DateTimeOffset recordedAt)
    {
        ValidateFacts(facts);
        var outputPath = ValidateOutputPath(path);
        var directory = Path.GetDirectoryName(outputPath)!;
        var temporary = Path.Combine(
            directory,
            $".{Path.GetFileName(outputPath)}.{Guid.NewGuid():N}.tmp");
        try
        {
            var options = new FileStreamOptions
            {
                Access = FileAccess.Write,
                Mode = FileMode.CreateNew,
                Share = FileShare.None,
                Options = FileOptions.WriteThrough,
            };
            if (!OperatingSystem.IsWindows())
            {
                options.UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
            }
            using (var stream = new FileStream(temporary, options))
            {
                using (var writer = new Utf8JsonWriter(stream, new JsonWriterOptions
                {
                    Indented = true,
                }))
                {
                    writer.WriteStartObject();
                    writer.WriteNumber("schema_version", 1);
                    writer.WriteString("proof", ProofId);
                    writer.WriteString("recorded_at", recordedAt.ToUniversalTime());
                    writer.WriteStartObject("source");
                    writer.WriteString("avalonia_contract", ContractVersion);
                    writer.WriteString("binary_sha256", facts.BinarySha256);
                    writer.WriteEndObject();
                    writer.WriteStartObject("registration");
                    writer.WriteString("application_key", SilvortexAccountOptions.ReviewedApplicationKey);
                    writer.WriteString("client_profile", SilvortexAccountOptions.ReviewedClientProfile);
                    writer.WriteString("client_id", SilvortexAccountOptions.ReviewedClientId);
                    writer.WriteString("client_kind", "native");
                    writer.WriteBoolean("public_client", true);
                    writer.WriteBoolean("client_secret_present", false);
                    writer.WriteString(
                        "redirect_uri",
                        "http://127.0.0.1:43817/oidc/callback");
                    writer.WriteString("scopes", SilvortexAccountOptions.ReviewedScopes);
                    writer.WriteEndObject();
                    writer.WriteStartObject("target");
                    writer.WriteString("operating_system", facts.OperatingSystem);
                    writer.WriteString("architecture", facts.Architecture);
                    writer.WriteString("configuration_source", facts.ConfigurationSource);
                    writer.WriteString("execution", "packaged-native-aot-system-browser");
                    writer.WriteBoolean("native_aot", true);
                    writer.WriteEndObject();
                    writer.WriteStartObject("observations");
                    writer.WriteBoolean("system_browser_launched", facts.SystemBrowserLaunched);
                    writer.WriteBoolean("loopback_callback_accepted", facts.LoopbackCallbackAccepted);
                    writer.WriteBoolean(
                        "platform_vault_login_persisted",
                        facts.PlatformVaultLoginPersisted);
                    writer.WriteBoolean("fresh_session_restore", facts.FreshSessionRestore);
                    writer.WriteBoolean(
                        "refresh_credential_rotated",
                        facts.RefreshCredentialRotated);
                    writer.WriteBoolean(
                        "access_credential_revocation_attempted",
                        facts.AccessCredentialRevocationAttempted);
                    writer.WriteBoolean(
                        "refresh_credential_revocation_attempted",
                        facts.RefreshCredentialRevocationAttempted);
                    writer.WriteBoolean("local_logout_completed", facts.LocalLogoutCompleted);
                    writer.WriteBoolean(
                        "platform_vault_empty_after_logout",
                        facts.PlatformVaultEmptyAfterLogout);
                    writer.WriteEndObject();
                    writer.WriteStartObject("boundaries");
                    writer.WriteBoolean("provider_origin_written", false);
                    writer.WriteBoolean("account_identity_written", false);
                    writer.WriteBoolean("credential_value_written", false);
                    writer.WriteBoolean("credential_digest_written", false);
                    writer.WriteBoolean("daemon_authority_touched", false);
                    writer.WriteBoolean("preexisting_credential_overwritten", false);
                    writer.WriteBoolean("environment_override_accepted", false);
                    writer.WriteBoolean("secret_free", true);
                    writer.WriteEndObject();
                    writer.WriteNumber("duration_ms", facts.DurationMilliseconds);
                    writer.WriteString("result", "passed");
                    writer.WriteEndObject();
                }
                stream.Flush(flushToDisk: true);
            }
            if (new FileInfo(temporary).Length is <= 0 or > MaxEvidenceBytes)
            {
                throw new InvalidDataException(
                    "Team Silvortex desktop proof evidence exceeds its size limit.");
            }
            File.Move(temporary, outputPath, overwrite: false);
        }
        finally
        {
            File.Delete(temporary);
        }
    }

    private static void ValidateFacts(ProofFacts facts)
    {
        if (facts.OperatingSystem is not ("macos" or "linux")
            || facts.Architecture.Length is <= 0 or > 32
            || (facts.OperatingSystem == "macos"
                && facts.ConfigurationSource != "packaged-info-plist")
            || (facts.OperatingSystem == "linux"
                && facts.ConfigurationSource != "environment")
            || facts.BinarySha256.Length != 64
            || !facts.BinarySha256.All(character => char.IsAsciiHexDigit(character))
            || facts.DurationMilliseconds <= 0
            || !facts.SystemBrowserLaunched
            || !facts.LoopbackCallbackAccepted
            || !facts.PlatformVaultLoginPersisted
            || !facts.FreshSessionRestore
            || !facts.RefreshCredentialRotated
            || !facts.AccessCredentialRevocationAttempted
            || !facts.RefreshCredentialRevocationAttempted
            || !facts.LocalLogoutCompleted
            || !facts.PlatformVaultEmptyAfterLogout)
        {
            throw new InvalidDataException(
                "Team Silvortex desktop proof facts are incomplete.");
        }
    }

    private static void ExpectRejected(Action action)
    {
        try
        {
            action();
        }
        catch (Exception error) when (error is InvalidDataException or IOException)
        {
            return;
        }
        throw new InvalidDataException(
            "Team Silvortex desktop proof accepted an unsafe verification case.");
    }

    private sealed record ProofFacts(
        string OperatingSystem,
        string Architecture,
        string ConfigurationSource,
        string BinarySha256,
        long DurationMilliseconds,
        bool SystemBrowserLaunched,
        bool LoopbackCallbackAccepted,
        bool PlatformVaultLoginPersisted,
        bool FreshSessionRestore,
        bool RefreshCredentialRotated,
        bool AccessCredentialRevocationAttempted,
        bool RefreshCredentialRevocationAttempted,
        bool LocalLogoutCompleted,
        bool PlatformVaultEmptyAfterLogout);
}
