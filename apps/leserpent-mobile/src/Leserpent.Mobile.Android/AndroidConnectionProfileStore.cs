using Android.Content;

public sealed class AndroidConnectionProfileStore
{
    private const string PreferencesName = "leserpent.remote.profile.v1";
    private const string EndpointKey = "endpoint";
    private readonly MobileConnectionProfileStore profiles;

    public AndroidConnectionProfileStore(Context context)
    {
        var applicationContext = context.ApplicationContext
            ?? throw new InvalidOperationException("Android application context is unavailable.");
        profiles = new MobileConnectionProfileStore(
            new AndroidEndpointStore(applicationContext),
            applicationContext.FilesDir?.AbsolutePath
                ?? throw new InvalidOperationException("Android private storage is unavailable."),
            applicationContext.CacheDir?.AbsolutePath
                ?? throw new InvalidOperationException("Android cache storage is unavailable."));
    }

    public MobileConnectionProfile? Load() => profiles.Load();

    public MobileConnectionProfile Save(string endpoint, string certificatePem) =>
        profiles.Save(endpoint, certificatePem);

    public string CertificateAuthorityPath(string endpoint) =>
        profiles.CertificateAuthorityPath(endpoint);

    public string CachePath(string endpoint) => profiles.CachePath(endpoint);

    private sealed class AndroidEndpointStore(Context applicationContext) : IMobileEndpointStore
    {
        public string? Load()
        {
            var preferences = applicationContext.GetSharedPreferences(
                PreferencesName,
                FileCreationMode.Private)
                ?? throw new InvalidOperationException("Android profile storage is unavailable.");
            return preferences.GetString(EndpointKey, null);
        }

        public void Save(string endpoint)
        {
            var preferences = applicationContext.GetSharedPreferences(
                PreferencesName,
                FileCreationMode.Private)
                ?? throw new InvalidOperationException("Android profile storage is unavailable.");
            using var editor = preferences.Edit()
                ?? throw new InvalidOperationException("Android profile storage is unavailable.");
            if (!editor.PutString(EndpointKey, endpoint)!.Commit())
            {
                throw new InvalidOperationException("Android profile storage rejected the write.");
            }
        }
    }
}
