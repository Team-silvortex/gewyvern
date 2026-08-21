using Foundation;

public sealed class IosConnectionProfileStore
{
    private readonly MobileConnectionProfileStore profiles = new MobileConnectionProfileStore(
        new IosEndpointStore(),
        PrivateDirectory(NSSearchPathDirectory.ApplicationSupportDirectory),
        PrivateDirectory(NSSearchPathDirectory.CachesDirectory));

    public MobileConnectionProfile? Load() => profiles.Load();

    public MobileConnectionProfile Save(string endpoint, string certificatePem) =>
        profiles.Save(endpoint, certificatePem);

    public string CertificateAuthorityPath(string endpoint) =>
        profiles.CertificateAuthorityPath(endpoint);

    public string CachePath(string endpoint) => profiles.CachePath(endpoint);

    private static string PrivateDirectory(NSSearchPathDirectory directory) =>
        NSSearchPath.GetDirectories(directory, NSSearchPathDomain.User, true)
            .SingleOrDefault()
        ?? throw new InvalidOperationException("iOS private storage is unavailable.");

    private sealed class IosEndpointStore : IMobileEndpointStore
    {
        private const string EndpointKey = "leserpent.remote.profile.v1.endpoint";

        public string? Load() => NSUserDefaults.StandardUserDefaults.StringForKey(EndpointKey);

        public void Save(string endpoint) =>
            NSUserDefaults.StandardUserDefaults.SetString(endpoint, EndpointKey);
    }
}
