using Android.App;
using Android.Content.PM;
using Android.Graphics;
using Android.OS;
using Android.Text;
using Android.Views;
using Android.Widget;

[assembly: UsesPermission(Android.Manifest.Permission.Internet)]

[Activity(
    Label = "Leserpent",
    MainLauncher = true,
    Exported = true,
    Theme = "@android:style/Theme.Material.NoActionBar",
    ConfigurationChanges = ConfigChanges.Orientation | ConfigChanges.ScreenSize)]
public sealed class MainActivity : Activity
{
    private readonly CancellationTokenSource lifetime = new();
    private AndroidConnectionProfileStore profileStore = null!;
    private MobileApplicationCoordinator coordinator = null!;
    private EditText endpointInput = null!;
    private EditText certificateInput = null!;
    private EditText tokenInput = null!;
    private TextView statusText = null!;
    private LinearLayout runtimeList = null!;
    private Button connectButton = null!;

    protected override void OnCreate(Bundle? savedInstanceState)
    {
        base.OnCreate(savedInstanceState);
        Window?.SetFlags(WindowManagerFlags.Secure, WindowManagerFlags.Secure);
        profileStore = new AndroidConnectionProfileStore(this);
        coordinator = new MobileApplicationCoordinator(
            new MobileCredentialVault(new AndroidKeystoreSecretStore(this)));
        coordinator.StateChanged += OnCoordinatorStateChanged;
        SetContentView(BuildContent());
        var profile = profileStore.Load();
        endpointInput.Text = profile?.Endpoint ?? string.Empty;
        statusText.Text = profile is null
            ? "Enter the HTTPS authority, CA certificate, and endpoint-scoped token."
            : "Saved profile loaded. Connecting when the app enters foreground.";
        connectButton.Click += async (_, _) => await ConfigureAndConnectAsync();
    }

    protected override async void OnStart()
    {
        base.OnStart();
        try
        {
            if (coordinator.State.Phase == MobileApplicationPhase.Unconfigured
                && profileStore.Load() is { } profile)
            {
                await coordinator.ConfigureAsync(
                    profile.Endpoint,
                    profile.CertificateAuthorityPath,
                    profileStore.CachePath(profile.Endpoint),
                    null,
                    lifetime.Token);
            }
            if (coordinator.State.Phase is MobileApplicationPhase.Inactive
                or MobileApplicationPhase.Background
                or MobileApplicationPhase.Faulted)
            {
                await coordinator.EnterForegroundAsync(lifetime.Token);
            }
        }
        catch (Exception error) when (error is not OperationCanceledException)
        {
            ShowError(error);
        }
    }

    protected override void OnStop()
    {
        try
        {
            coordinator.EnterBackgroundAsync().AsTask().GetAwaiter().GetResult();
        }
        catch (Exception error) when (error is not ObjectDisposedException)
        {
            ShowError(error);
        }
        base.OnStop();
    }

    protected override void OnDestroy()
    {
        lifetime.Cancel();
        coordinator.StateChanged -= OnCoordinatorStateChanged;
        coordinator.DisposeAsync().AsTask().GetAwaiter().GetResult();
        lifetime.Dispose();
        base.OnDestroy();
    }

    private async Task ConfigureAndConnectAsync()
    {
        connectButton.Enabled = false;
        try
        {
            var endpoint = endpointInput.Text?.Trim() ?? string.Empty;
            var certificatePem = certificateInput.Text;
            if (string.IsNullOrWhiteSpace(certificatePem))
            {
                var saved = profileStore.Load();
                certificatePem = saved is not null
                    && RemoteClientOptions.ParseEndpoint(endpoint).AbsoluteUri == saved.Endpoint
                    ? File.ReadAllText(saved.CertificateAuthorityPath)
                    : throw new InvalidDataException(
                        "A CA certificate is required when configuring a new endpoint.");
            }
            var profile = profileStore.Save(endpoint, certificatePem);
            var token = string.IsNullOrEmpty(tokenInput.Text) ? null : tokenInput.Text;
            await coordinator.ConfigureAsync(
                profile.Endpoint,
                profile.CertificateAuthorityPath,
                profileStore.CachePath(profile.Endpoint),
                token,
                lifetime.Token);
            tokenInput.Text = string.Empty;
            certificateInput.Text = string.Empty;
            await coordinator.EnterForegroundAsync(lifetime.Token);
        }
        catch (Exception error) when (error is not OperationCanceledException)
        {
            ShowError(error);
        }
        finally
        {
            connectButton.Enabled = true;
        }
    }

    private View BuildContent()
    {
        var body = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
        };
        var padding = Dp(20);
        body.SetPadding(padding, padding, padding, padding);
        body.AddView(Text("LESERPENT", 13, "#FFB229"));
        body.AddView(Text("Remote control, without semantic forks", 24, "#F4C95D"));
        body.AddView(Text("HTTPS authority", 14, "#E9E1D0"));
        endpointInput = Input("https://control.example:9443");
        endpointInput.InputType = InputTypes.ClassText | InputTypes.TextVariationUri;
        body.AddView(endpointInput);
        body.AddView(Text("CA certificate PEM", 14, "#E9E1D0"));
        certificateInput = Input("Paste on first setup; leave empty to reuse the saved CA");
        certificateInput.SetMinLines(3);
        certificateInput.SetMaxLines(8);
        certificateInput.InputType = InputTypes.ClassText | InputTypes.TextFlagMultiLine;
        body.AddView(certificateInput);
        body.AddView(Text("Endpoint-scoped token", 14, "#E9E1D0"));
        tokenInput = Input("Stored only in Android Keystore");
        tokenInput.InputType = InputTypes.ClassText | InputTypes.TextVariationPassword;
        body.AddView(tokenInput);
        connectButton = new Button(this)
        {
            Text = "Save and connect",
        };
        body.AddView(connectButton, MatchWidth());
        statusText = Text(string.Empty, 14, "#B9AA8A");
        statusText.SetPadding(0, Dp(12), 0, Dp(12));
        body.AddView(statusText, MatchWidth());
        body.AddView(Text("Runtimes", 19, "#F4C95D"));
        runtimeList = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
        };
        body.AddView(runtimeList, MatchWidth());
        var scroll = new ScrollView(this)
        {
            Background = new Android.Graphics.Drawables.ColorDrawable(Color.ParseColor("#11100D")),
            FillViewport = true,
            LayoutParameters = MatchWidth(),
        };
        scroll.AddView(body);
        return scroll;
    }

    private void OnCoordinatorStateChanged(MobileApplicationSnapshot snapshot) =>
        RunOnUiThread(() => Render(snapshot));

    private void Render(MobileApplicationSnapshot snapshot)
    {
        statusText.Text = snapshot.Error ?? snapshot.Remote?.Feed.Detail ?? snapshot.Phase.ToString();
        runtimeList.RemoveAllViews();
        var runtimes = snapshot.Remote?.Feed.Runtimes ?? [];
        if (runtimes.Count == 0)
        {
            runtimeList.AddView(Text("No runtime projection available.", 14, "#B9AA8A"));
            return;
        }
        foreach (var runtime in runtimes)
        {
            runtimeList.AddView(Text(
                $"{Safe(runtime.Name)}\n{runtime.Id} / {runtime.RefreshStatus}",
                15,
                "#E9E1D0"));
        }
    }

    private void ShowError(Exception error) => RunOnUiThread(() =>
    {
        statusText.Text = $"Connection blocked: {Safe(error.Message)}";
        statusText.SetTextColor(Color.ParseColor("#FF8A65"));
    });

    private EditText Input(string hint)
    {
        var input = new EditText(this)
        {
            Hint = hint,
            BackgroundTintList = Android.Content.Res.ColorStateList.ValueOf(
                Color.ParseColor("#FF9418")),
            LayoutParameters = MatchWidth(),
        };
        input.SetTextColor(Color.ParseColor("#E9E1D0"));
        input.SetHintTextColor(Color.ParseColor("#8F826B"));
        return input;
    }

    private TextView Text(string value, float size, string color)
    {
        var text = new TextView(this)
        {
            Text = value,
            TextSize = size,
            LayoutParameters = MatchWidth(),
        };
        text.SetTextColor(Color.ParseColor(color));
        return text;
    }

    private static LinearLayout.LayoutParams MatchWidth() => new(
        ViewGroup.LayoutParams.MatchParent,
        ViewGroup.LayoutParams.WrapContent);

    private int Dp(int value) => (int)(value * Resources!.DisplayMetrics!.Density + 0.5f);

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
