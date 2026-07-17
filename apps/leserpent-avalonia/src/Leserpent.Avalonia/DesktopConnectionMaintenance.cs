internal static class DesktopConnectionMaintenance
{
    public static void ForgetSavedConnection(
        DesktopConnectionProfile expectedProfile,
        DesktopConnectionProfileStore profileStore,
        IRemoteTokenVault? tokenVault = null)
    {
        var savedProfile = profileStore.Load()
            ?? throw new InvalidDataException("the saved desktop connection no longer exists");
        if (savedProfile != expectedProfile)
        {
            throw new InvalidDataException(
                "the saved desktop connection changed; reopen connection settings before forgetting it");
        }

        var endpoint = RemoteClientOptions.ParseEndpoint(savedProfile.Endpoint);
        RemoteTokenResolver.Delete(endpoint, tokenVault);
        profileStore.Clear();
    }

    public static void VerifyContract()
    {
        var root = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-connection-maintenance-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            var certificate = Path.Combine(root, "ca.pem");
            File.WriteAllText(certificate, "bounded certificate fixture");
            var profile = new DesktopConnectionProfile
            {
                SchemaVersion = 1,
                Endpoint = "https://control.example:9443",
                CertificateAuthorityPath = certificate,
            };
            var store = new DesktopConnectionProfileStore(Path.Combine(root, "profile.json"));
            var vault = new VerifyingTokenVault();
            store.Save(profile);
            vault.Store(RemoteClientOptions.ParseEndpoint(profile.Endpoint), new string('v', 32));

            ForgetSavedConnection(profile, store, vault);
            if (store.Load() is not null
                || vault.Load(RemoteClientOptions.ParseEndpoint(profile.Endpoint)) is not null
                || vault.DeleteCount != 1)
            {
                throw new InvalidDataException("saved connection was not forgotten completely");
            }

            store.Save(profile);
            var staleProfile = profile with { Endpoint = "https://other.example:9443" };
            try
            {
                ForgetSavedConnection(staleProfile, store, vault);
            }
            catch (InvalidDataException)
            {
                if (store.Load() != profile || vault.DeleteCount != 1)
                {
                    throw new InvalidDataException(
                        "stale connection protection modified persisted state");
                }
                return;
            }
            throw new InvalidDataException("stale connection was allowed to delete credentials");
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    private sealed class VerifyingTokenVault : IRemoteTokenVault
    {
        private readonly Dictionary<string, string> tokens = new(StringComparer.Ordinal);

        public int DeleteCount { get; private set; }

        public string? Load(Uri endpoint) =>
            tokens.GetValueOrDefault(RemoteTokenResolver.Account(endpoint));

        public void Store(Uri endpoint, string token) =>
            tokens[RemoteTokenResolver.Account(endpoint)] = token;

        public void Delete(Uri endpoint)
        {
            DeleteCount++;
            tokens.Remove(RemoteTokenResolver.Account(endpoint));
        }
    }
}
