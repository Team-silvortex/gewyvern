using Android.App;
using Android.Content;
using Android.Content.PM;
using Android.Graphics;
using Android.Graphics.Drawables;
using Android.OS;
using Android.Text;
using Android.Views;
using Android.Widget;
using AndroidConfiguration = Android.Content.Res.Configuration;

[assembly: UsesPermission(Android.Manifest.Permission.Internet)]

[Activity(
    Label = "Leserpent",
    Icon = "@mipmap/leserpent_icon",
    MainLauncher = true,
    Exported = true,
    Theme = "@android:style/Theme.Material.NoActionBar",
    ConfigurationChanges = ConfigChanges.Orientation
        | ConfigChanges.ScreenSize
        | ConfigChanges.SmallestScreenSize)]
public sealed class MainActivity : Activity
{
    private readonly CancellationTokenSource lifetime = new();
    private AndroidConnectionProfileStore profileStore = null!;
    private MobileApplicationCoordinator coordinator = null!;
    private EditText endpointInput = null!;
    private EditText certificateInput = null!;
    private EditText tokenInput = null!;
    private TextView statusText = null!;
    private TextView runtimeHeading = null!;
    private LinearLayout runtimeList = null!;
    private Button connectButton = null!;
    private Button editConnectionButton = null!;
    private Button backToFleetButton = null!;
    private TextView connectionSummaryText = null!;
    private LinearLayout rootLayout = null!;
    private LinearLayout body = null!;
    private LinearLayout contentColumns = null!;
    private LinearLayout connectionHeader = null!;
    private LinearLayout connectionIdentity = null!;
    private LinearLayout connectionSection = null!;
    private LinearLayout connectionFields = null!;
    private LinearLayout runtimeSection = null!;
    private LinearLayout actionBar = null!;
    private MobileSafeAreaInsets safeArea = MobileSafeAreaInsets.Zero;
    private double imeBottomDp;
    private MobileLayoutPlan? appliedLayoutPlan;
    private int appliedContentWidth = int.MinValue;
    private bool connectionExpanded;
    private int runtimeColumnCount = 1;
    private LinearLayout runtimeColumnHost = null!;
    private readonly MobileNativeRenderGate renderGate = new();
    private MobileUiDocumentBinding? activeDocument;
    private string? activeWorkspaceRuntimeId;
    private string? startupStatus;
    private string? operationStatus;
    private bool operationFailed;
    private bool operationBusy;

    protected override void OnCreate(Bundle? savedInstanceState)
    {
        base.OnCreate(savedInstanceState);
#if !DEBUG || !LESERPENT_UI_CAPTURE
        Window?.SetFlags(WindowManagerFlags.Secure, WindowManagerFlags.Secure);
#endif
        profileStore = new AndroidConnectionProfileStore(this);
        coordinator = new MobileApplicationCoordinator(
            new MobileCredentialVault(new AndroidKeystoreSecretStore(this)));
        coordinator.StateChanged += OnCoordinatorStateChanged;
        if (OperatingSystem.IsAndroidVersionAtLeast(30)
            && !OperatingSystem.IsAndroidVersionAtLeast(35))
        {
#pragma warning disable CS0618 // Android 15 enforces edge-to-edge; API 30-34 needs this bridge.
            Window?.SetDecorFitsSystemWindows(false);
#pragma warning restore CS0618
        }
        SetContentView(BuildContent());
        rootLayout.RequestApplyInsets();
        var profile = profileStore.Load();
        endpointInput.Text = profile?.Endpoint ?? string.Empty;
        UpdateConnectionSummary(profile?.Endpoint);
        SetConnectionExpanded(profile is null);
        startupStatus = profile is null
            ? "Enter the HTTPS authority, CA certificate, and endpoint-scoped token."
            : "Saved profile loaded. Connecting when the app enters foreground.";
        operationFailed = false;
        connectButton.Click += async (_, _) => await ConfigureAndConnectAsync();
        editConnectionButton.Click += (_, _) => SetConnectionExpanded(!connectionExpanded);
        Render(coordinator.State);
        rootLayout.Post(ApplyLayout);
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
        catch (Exception error) when (error is not System.OperationCanceledException)
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

    public override void OnConfigurationChanged(AndroidConfiguration newConfig)
    {
        base.OnConfigurationChanged(newConfig);
        rootLayout.Post(ApplyLayout);
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
        SetConnectEnabled(false);
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
            activeDocument = null;
            activeWorkspaceRuntimeId = null;
            operationStatus = null;
            await coordinator.EnterForegroundAsync(lifetime.Token);
            UpdateConnectionSummary(profile.Endpoint);
            SetConnectionExpanded(false);
        }
        catch (Exception error) when (error is not System.OperationCanceledException)
        {
            ShowError(error);
        }
        finally
        {
            SetConnectEnabled(true);
        }
    }

    private View BuildContent()
    {
        rootLayout = new AdaptiveRootLayout(this, UpdateWindowInsets)
        {
            Orientation = Orientation.Vertical,
            Background = new ColorDrawable(Color.ParseColor("#11100D")),
        };
        var scroll = new ScrollView(this)
        {
            FillViewport = true,
        };
        scroll.SetClipToPadding(false);
        var contentHost = new FrameLayout(this);
        body = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
        };
        contentHost.AddView(body);
        scroll.AddView(contentHost, new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MatchParent,
            ViewGroup.LayoutParams.WrapContent));
        rootLayout.AddView(scroll, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MatchParent,
            0,
            1));

        body.AddView(Text("LESERPENT", 13, "#FFB229"), MatchWidth());
        var heading = Text("Remote control, without semantic forks", 26, "#F4C95D");
        heading.SetTypeface(null, Android.Graphics.TypefaceStyle.Bold);
        AddWithTopMargin(body, heading, 4);
        var subcopy = Text(
            "One secure authority at a time. Connection setup stays out of the way after onboarding.",
            14,
            "#B9AA8A");
        AddWithTopMargin(body, subcopy, 8);

        contentColumns = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
        };
        AddWithTopMargin(body, contentColumns, 20);

        connectionSection = Section();
        connectionHeader = new LinearLayout(this)
        {
            Orientation = Orientation.Horizontal,
        };
        connectionHeader.SetGravity(GravityFlags.CenterVertical);
        connectionIdentity = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
        };
        var connectionHeading = Text("Authority", 18, "#F4C95D");
        connectionHeading.SetTypeface(null, Android.Graphics.TypefaceStyle.Bold);
        connectionIdentity.AddView(connectionHeading, MatchWidth());
        connectionSummaryText = Text(string.Empty, 13, "#B9AA8A");
        AddWithTopMargin(connectionIdentity, connectionSummaryText, 4);
        editConnectionButton = new Button(this)
        {
            Text = "Edit connection",
            ContentDescription = "Edit the saved authority connection",
        };
        connectionHeader.AddView(connectionIdentity, new LinearLayout.LayoutParams(
            0,
            ViewGroup.LayoutParams.WrapContent,
            1));
        connectionHeader.AddView(editConnectionButton, WrapContent());
        connectionSection.AddView(connectionHeader, MatchWidth());

        connectionFields = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
        };
        AddWithTopMargin(connectionSection, connectionFields, 16);
        connectionFields.AddView(Text("HTTPS authority", 14, "#E9E1D0"), MatchWidth());
        endpointInput = Input("https://control.example:9443");
        endpointInput.InputType = InputTypes.ClassText | InputTypes.TextVariationUri;
        endpointInput.ContentDescription = "HTTPS daemon authority";
        AddWithTopMargin(connectionFields, endpointInput, 4);
        AddWithTopMargin(
            connectionFields,
            Text("CA certificate PEM", 14, "#E9E1D0"),
            12);
        certificateInput = Input("Paste on first setup; leave empty to reuse the saved CA");
        certificateInput.SetMinLines(3);
        certificateInput.SetMaxLines(8);
        certificateInput.InputType = InputTypes.ClassText | InputTypes.TextFlagMultiLine;
        certificateInput.ContentDescription = "Public CA certificate in PEM format";
        AddWithTopMargin(connectionFields, certificateInput, 4);
        AddWithTopMargin(
            connectionFields,
            Text("Endpoint-scoped token", 14, "#E9E1D0"),
            12);
        tokenInput = Input("Keystore protected");
        tokenInput.InputType = InputTypes.ClassText | InputTypes.TextVariationPassword;
        tokenInput.ContentDescription = "Endpoint-scoped token stored in Android Keystore";
        AddWithTopMargin(connectionFields, tokenInput, 4);
        contentColumns.AddView(connectionSection, MatchWidth());

        runtimeSection = Section();
        var runtimeHeader = new LinearLayout(this)
        {
            Orientation = Orientation.Horizontal,
        };
        runtimeHeader.SetGravity(GravityFlags.CenterVertical);
        runtimeHeading = Text("Runtimes", 19, "#F4C95D");
        runtimeHeading.SetTypeface(null, Android.Graphics.TypefaceStyle.Bold);
        backToFleetButton = new Button(this)
        {
            Text = "Back to fleet",
            ContentDescription = "Return to the remote runtime fleet",
            Visibility = ViewStates.Gone,
        };
        backToFleetButton.Click += (_, _) => ReturnToFleet();
        runtimeHeader.AddView(runtimeHeading, new LinearLayout.LayoutParams(
            0,
            ViewGroup.LayoutParams.WrapContent,
            1));
        runtimeHeader.AddView(backToFleetButton, WrapContent());
        runtimeSection.AddView(runtimeHeader, MatchWidth());
        statusText = Text(string.Empty, 14, "#B9AA8A");
        statusText.AccessibilityLiveRegion = AccessibilityLiveRegion.Polite;
        AddWithTopMargin(runtimeSection, statusText, 8);
        runtimeList = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
        };
        AddWithTopMargin(runtimeSection, runtimeList, 14);
        contentColumns.AddView(runtimeSection, MatchWidth());

        actionBar = new LinearLayout(this)
        {
            Orientation = Orientation.Horizontal,
            Background = new ColorDrawable(Color.ParseColor("#1A1712")),
            Elevation = Dp(8),
        };
        actionBar.SetGravity(GravityFlags.CenterVertical | GravityFlags.End);
        connectButton = new Button(this)
        {
            Text = "Save and connect",
            ContentDescription = "Save the authority and connect",
        };
        SetConnectEnabled(true);
        actionBar.AddView(connectButton, WrapContent());
        rootLayout.AddView(actionBar, MatchWidth());
        return rootLayout;
    }

    private void ApplyLayout()
    {
        if (IsFinishing || IsDestroyed)
        {
            return;
        }
        var configuration = Resources?.Configuration
            ?? throw new InvalidDataException("Android configuration is unavailable");
        var metrics = Resources?.DisplayMetrics
            ?? throw new InvalidDataException("Android display metrics are unavailable");
        var widthDp = configuration.ScreenWidthDp > 0
            ? configuration.ScreenWidthDp
            : metrics.WidthPixels / metrics.Density;
        var heightDp = configuration.ScreenHeightDp > 0
            ? configuration.ScreenHeightDp
            : metrics.HeightPixels / metrics.Density;
        var fontScale = configuration.FontScale > 0 ? configuration.FontScale : 1;
        var plan = MobileLayoutPolicy.Resolve(
            widthDp,
            heightDp,
            fontScale,
            safeArea);

        var contentWidth = plan.ContentMaxWidthDp == 0
            ? ViewGroup.LayoutParams.MatchParent
            : Math.Min(Dp(plan.ContentMaxWidthDp), metrics.WidthPixels);
        body.SetPadding(
            Dp(plan.ContentInsets.Left),
            Dp(plan.ContentInsets.Top),
            Dp(plan.ContentInsets.Right),
            Dp(connectionExpanded
                ? plan.SectionSpacingDp
                : plan.ContentInsets.Bottom));
        var actionBottom = Math.Max(
            plan.ActionInsets.Bottom,
            checked((int)Math.Ceiling(imeBottomDp) + 10));
        actionBar.SetPadding(
            Dp(plan.ActionInsets.Left),
            Dp(plan.ActionInsets.Top),
            Dp(plan.ActionInsets.Right),
            Dp(actionBottom));
        if (appliedLayoutPlan == plan && appliedContentWidth == contentWidth)
        {
            return;
        }
        appliedLayoutPlan = plan;
        appliedContentWidth = contentWidth;
        body.LayoutParameters = new FrameLayout.LayoutParams(
            contentWidth,
            ViewGroup.LayoutParams.WrapContent,
            GravityFlags.Top | GravityFlags.CenterHorizontal);

        contentColumns.Orientation = plan.TwoPane
            ? Orientation.Horizontal
            : Orientation.Vertical;
        if (plan.TwoPane)
        {
            connectionSection.LayoutParameters = WeightedSection(5, rightMargin: 10);
            runtimeSection.LayoutParameters = WeightedSection(7, leftMargin: 10);
        }
        else
        {
            connectionSection.LayoutParameters = MatchWidth();
            var runtimeParameters = MatchWidth();
            runtimeParameters.TopMargin = Dp(plan.SectionSpacingDp);
            runtimeSection.LayoutParameters = runtimeParameters;
        }

        var compact = plan.WidthClass == MobileWidthClass.Compact;
        connectionHeader.Orientation = compact
            ? Orientation.Vertical
            : Orientation.Horizontal;
        connectionHeader.SetGravity(compact
            ? GravityFlags.Start
            : GravityFlags.CenterVertical);
        connectionIdentity.LayoutParameters = compact
            ? MatchWidth()
            : new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WrapContent, 1);
        var editParameters = compact ? MatchWidth() : WrapContent();
        editParameters.TopMargin = compact ? Dp(8) : 0;
        editConnectionButton.LayoutParameters = editParameters;
        connectButton.LayoutParameters = compact ? MatchWidth() : WrapContent();

        var minimumTouchHeight = Dp(plan.MinimumTouchTargetDp);
        endpointInput.SetMinimumHeight(minimumTouchHeight);
        certificateInput.SetMinimumHeight(minimumTouchHeight);
        tokenInput.SetMinimumHeight(minimumTouchHeight);
        editConnectionButton.SetMinimumHeight(minimumTouchHeight);
        backToFleetButton.SetMinimumHeight(minimumTouchHeight);
        connectButton.SetMinimumHeight(minimumTouchHeight);
        if (runtimeColumnCount != plan.RuntimeColumns)
        {
            runtimeColumnCount = plan.RuntimeColumns;
            Render(coordinator.State);
        }
    }

    private void UpdateWindowInsets(WindowInsets insets)
    {
        if (!OperatingSystem.IsAndroidVersionAtLeast(30))
        {
            safeArea = MobileSafeAreaInsets.Zero;
            imeBottomDp = 0;
            return;
        }
        var bars = insets.GetInsets(
            WindowInsets.Type.SystemBars() | WindowInsets.Type.DisplayCutout());
        var ime = insets.GetInsets(WindowInsets.Type.Ime());
        safeArea = new MobileSafeAreaInsets(
            PxToDp(bars.Left),
            PxToDp(bars.Top),
            PxToDp(bars.Right),
            PxToDp(bars.Bottom));
        imeBottomDp = PxToDp(ime.Bottom);
        rootLayout.Post(ApplyLayout);
    }

    private void SetConnectionExpanded(bool expanded)
    {
        connectionExpanded = expanded;
        connectionFields.Visibility = expanded ? ViewStates.Visible : ViewStates.Gone;
        actionBar.Visibility = expanded ? ViewStates.Visible : ViewStates.Gone;
        editConnectionButton.Text = expanded ? "Hide setup" : "Edit connection";
        editConnectionButton.ContentDescription = expanded
            ? "Hide authority connection setup"
            : "Edit the saved authority connection";
        rootLayout.Post(ApplyLayout);
    }

    private void UpdateConnectionSummary(string? endpoint)
    {
        connectionSummaryText.Text = string.IsNullOrWhiteSpace(endpoint)
            ? "Not configured"
            : Safe(endpoint);
    }

    private void OnCoordinatorStateChanged(MobileApplicationSnapshot snapshot) =>
        RunOnUiThread(() => Render(snapshot));

    private void Render(MobileApplicationSnapshot snapshot)
    {
        try
        {
            var feed = snapshot.Remote?.Feed ?? RemoteFeedState.Initial;
            string? documentStatus = null;
            if (activeWorkspaceRuntimeId is null)
            {
                var candidate = MobileUiDocumentBinding.Project(
                    RemoteDocumentProjection.Project(feed).Document);
                documentStatus = candidate.Find("remote-state")?.Text;
                activeDocument = MobileNativeRenderGate.RetainEquivalentPresentation(
                    activeDocument,
                    candidate,
                    "remote-state");
            }
            var document = activeDocument
                ?? throw new InvalidDataException("mobile UI document is unavailable");
            var heading = document.Root.Children.FirstOrDefault(node =>
                node.Kind == UiNodeKind.Heading)?.Text;
            runtimeHeading.Text = Safe(heading ?? (activeWorkspaceRuntimeId is null
                ? "Remote runtimes"
                : activeWorkspaceRuntimeId));
            backToFleetButton.Visibility = activeWorkspaceRuntimeId is null
                ? ViewStates.Gone
                : ViewStates.Visible;
            statusText.Text = Safe(
                operationStatus
                ?? snapshot.Error
                ?? (snapshot.Phase == MobileApplicationPhase.Unconfigured
                    ? startupStatus
                    : null)
                ?? documentStatus
                ?? snapshot.Remote?.Feed.Detail
                ?? snapshot.Phase.ToString());
            statusText.SetTextColor(Color.ParseColor(
                operationFailed || snapshot.Error is not null ? "#FF8A65" : "#B9AA8A"));

            if (!renderGate.ShouldRender(
                    document,
                    coordinator.MutationAvailability,
                    operationBusy,
                    runtimeColumnCount))
            {
                return;
            }
            runtimeList.RemoveAllViews();
            runtimeList.Orientation = Orientation.Vertical;
            if (document.Root.Kind == UiNodeKind.Column)
            {
                RenderFleetDocument(document, snapshot);
            }
            else
            {
                foreach (var node in document.Root.Children.Where(node =>
                             node.Kind != UiNodeKind.Heading))
                {
                    AddProjectedView(runtimeList, RenderNode(document, node, snapshot), 8);
                }
            }
        }
        catch (InvalidDataException error)
        {
            statusText.Text = $"UI document rejected: {Safe(error.Message)}";
            statusText.SetTextColor(Color.ParseColor("#FF8A65"));
            renderGate.Invalidate();
            runtimeList.RemoveAllViews();
            runtimeList.AddView(EmptyProjection("Remote projection is unavailable."), MatchWidth());
        }
    }

    private void RenderFleetDocument(
        MobileUiDocumentBinding document,
        MobileApplicationSnapshot snapshot)
    {
        var cards = document.Root.Children
            .Where(node => node.Kind == UiNodeKind.RuntimeCard)
            .ToArray();
        foreach (var node in document.Root.Children.Where(node =>
                     node.Kind is not (UiNodeKind.Heading or UiNodeKind.RuntimeCard)
                     && node.Id != "remote-state"))
        {
            AddProjectedView(runtimeList, RenderNode(document, node, snapshot), 6);
        }
        if (cards.Length == 0)
        {
            runtimeList.AddView(EmptyProjection("No runtime projection available."), MatchWidth());
            return;
        }
        runtimeColumnHost = new LinearLayout(this)
        {
            Orientation = runtimeColumnCount == 1
                ? Orientation.Vertical
                : Orientation.Horizontal,
        };
        AddProjectedView(runtimeList, runtimeColumnHost, 8);
        var columns = RuntimeColumns();
        for (var index = 0; index < cards.Length; index++)
        {
            var parameters = MatchWidth();
            parameters.BottomMargin = Dp(10);
            columns[index % columns.Count].AddView(
                RenderNode(document, cards[index], snapshot),
                parameters);
        }
    }

    private View RenderNode(
        MobileUiDocumentBinding document,
        MobileUiNodeBinding node,
        MobileApplicationSnapshot snapshot) => node.Kind switch
        {
            UiNodeKind.RuntimeCard or UiNodeKind.Section =>
                RenderContainer(document, node, snapshot),
            UiNodeKind.Action => RenderAction(document, node, snapshot),
            UiNodeKind.Column or UiNodeKind.RuntimeWorkspace =>
                RenderContainer(document, node, snapshot),
            UiNodeKind.Heading => ProjectedText(node, 17, "#F4C95D", bold: true),
            UiNodeKind.LogEntry => ProjectedText(node, 13, LogColor(node.Text), false),
            UiNodeKind.HistoryEntry => ProjectedText(node, 13, "#CFC4AE", false),
            _ => ProjectedText(node, 14, "#E9E1D0", false),
        };

    private LinearLayout RenderContainer(
        MobileUiDocumentBinding document,
        MobileUiNodeBinding node,
        MobileApplicationSnapshot snapshot)
    {
        var container = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
            Background = PanelBackground(node.Kind == UiNodeKind.RuntimeCard
                ? "#1C1913"
                : "#201C15"),
        };
        container.SetPadding(Dp(14), Dp(13), Dp(14), Dp(13));
        container.ContentDescription = Safe(
            node.AccessibleName ?? node.Text ?? node.Id);
        if (node.Text is { } title)
        {
            var heading = Text(Safe(title), 15, "#F4C95D");
            heading.SetTypeface(null, Android.Graphics.TypefaceStyle.Bold);
            container.AddView(heading, MatchWidth());
        }
        foreach (var child in node.Children)
        {
            AddProjectedView(container, RenderNode(document, child, snapshot), 7);
        }
        return container;
    }

    private TextView ProjectedText(
        MobileUiNodeBinding node,
        float size,
        string color,
        bool bold)
    {
        var text = Text(Safe(node.Text ?? node.AccessibleName ?? "Unavailable"), size, color);
        text.ContentDescription = Safe(node.AccessibleName ?? node.Text ?? node.Id);
        if (bold)
        {
            text.SetTypeface(null, Android.Graphics.TypefaceStyle.Bold);
        }
        return text;
    }

    private Button RenderAction(
        MobileUiDocumentBinding document,
        MobileUiNodeBinding node,
        MobileApplicationSnapshot snapshot)
    {
        var availability = coordinator.MutationAvailability;
        var enabled = !operationBusy && ActionEnabled(node.ActionKind, availability);
        var reason = ActionUnavailableReason(node.ActionKind, availability);
        var button = new Button(this)
        {
            Text = Safe(node.Text ?? node.AccessibleName ?? "Action"),
            Enabled = enabled,
            ContentDescription = Safe(string.Join(". ", new[]
            {
                node.AccessibleName ?? node.Text,
                node.AccessibleDescription,
                enabled ? null : reason,
            }.Where(value => !string.IsNullOrWhiteSpace(value)))),
        };
        _ = snapshot;
        button.SetMinimumHeight(Dp(MobileLayoutPolicy.MinimumTouchTargetDp));
        button.BackgroundTintList = Android.Content.Res.ColorStateList.ValueOf(
            Color.ParseColor(enabled ? "#FFB229" : "#5A5142"));
        button.SetTextColor(Color.ParseColor(enabled ? "#11100D" : "#D3C8B2"));
        button.Click += async (_, _) => await InvokeActionAsync(document, node);
        return button;
    }

    private IReadOnlyList<LinearLayout> RuntimeColumns()
    {
        if (runtimeColumnCount == 1)
        {
            return [runtimeColumnHost];
        }
        var left = new LinearLayout(this) { Orientation = Orientation.Vertical };
        var right = new LinearLayout(this) { Orientation = Orientation.Vertical };
        var leftParameters = new LinearLayout.LayoutParams(
            0,
            ViewGroup.LayoutParams.WrapContent,
            1)
        {
            RightMargin = Dp(5),
        };
        var rightParameters = new LinearLayout.LayoutParams(
            0,
            ViewGroup.LayoutParams.WrapContent,
            1)
        {
            LeftMargin = Dp(5),
        };
        runtimeColumnHost.AddView(left, leftParameters);
        runtimeColumnHost.AddView(right, rightParameters);
        return [left, right];
    }

    private async Task InvokeActionAsync(
        MobileUiDocumentBinding source,
        MobileUiNodeBinding node)
    {
        if (operationBusy || !ReferenceEquals(activeDocument, source))
        {
            return;
        }
        var feed = coordinator.State.Remote?.Feed;
        if (feed is null)
        {
            SetOperationStatus("Remote action blocked: no remote feed is available.", true);
            return;
        }
        var activation = source.ResolveActivation(
            node.Id,
            feed,
            coordinator.MutationAvailability);
        if (!activation.Accepted || activation.Intent is not { } intent)
        {
            SetOperationStatus($"Remote action blocked: {Safe(activation.Reason ?? "unavailable")}", true);
            return;
        }
        if (intent.Kind == ActionKind.RuntimeInspect)
        {
            await OpenWorkspaceAsync(intent.Runtime);
            return;
        }
        if (intent.Kind == ActionKind.RuntimeDeploy)
        {
            if (node.Form is not { } form)
            {
                SetOperationStatus("Deployment blocked: the shared form is unavailable.", true);
                return;
            }
            var values = await ShowParameterizedFormAsync(source, node, form, intent.Runtime);
            if (values is null || !ReferenceEquals(activeDocument, source))
            {
                return;
            }
            var currentFeed = coordinator.State.Remote?.Feed;
            if (currentFeed is null)
            {
                SetOperationStatus("Deployment blocked: the remote feed retired.", true);
                return;
            }
            var submitted = source.ResolveSubmission(node.Id, values, currentFeed);
            if (!submitted.Accepted || submitted.Intent is not { } submittedIntent)
            {
                SetOperationStatus(
                    $"Deployment blocked: {Safe(submitted.Reason ?? "invalid form event")}",
                    true);
                return;
            }
            intent = submittedIntent;
        }
        var confirmed = await ShowConfirmationAsync(
            ConfirmationTitle(intent.Kind),
            ConfirmationMessage(intent),
            ConfirmationAction(intent.Kind));
        if (!confirmed || lifetime.IsCancellationRequested)
        {
            return;
        }
        await ExecuteMutationAsync(intent);
    }

    private async Task OpenWorkspaceAsync(RemoteRuntimeProjection runtime)
    {
        operationBusy = true;
        SetOperationStatus($"Loading {Safe(runtime.Name)} workspace...", false);
        try
        {
            var workspace = await coordinator.LoadWorkspaceAsync(runtime.Id, lifetime.Token);
            if (lifetime.IsCancellationRequested)
            {
                return;
            }
            activeDocument = MobileUiDocumentBinding.Project(
                RemoteWorkspaceDocumentProjection.Project(workspace));
            activeWorkspaceRuntimeId = runtime.Id;
            operationStatus = $"Live workspace at revision {workspace.Revision}";
            operationFailed = false;
        }
        catch (System.OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            operationStatus = "Workspace request stopped when the app left foreground.";
            operationFailed = false;
        }
        catch (Exception error) when (error is not System.OperationCanceledException)
        {
            operationStatus = WorkspaceFailure(error);
            operationFailed = true;
        }
        finally
        {
            operationBusy = false;
            Render(coordinator.State);
        }
    }

    private async Task ExecuteMutationAsync(RemoteUiActionIntent intent)
    {
        operationBusy = true;
        SetOperationStatus(MutationProgress(intent), false);
        try
        {
            var result = await coordinator.ExecuteMutationAsync(intent, lifetime.Token);
            operationStatus = $"{MutationLabel(intent.Kind)} applied at revision {result.Revision}.";
            operationFailed = false;
        }
        catch (System.OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            operationStatus = "Remote change stopped when the app left foreground.";
            operationFailed = false;
        }
        catch (Exception error) when (error is not System.OperationCanceledException)
        {
            operationStatus = MutationFailure(error);
            operationFailed = true;
        }
        finally
        {
            operationBusy = false;
            Render(coordinator.State);
        }
    }

    private async Task<IReadOnlyDictionary<string, string>?> ShowParameterizedFormAsync(
        MobileUiDocumentBinding source,
        MobileUiNodeBinding node,
        MobileUiFormBinding form,
        RemoteRuntimeProjection runtime)
    {
        var completion = new TaskCompletionSource<IReadOnlyDictionary<string, string>?>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var fields = new Dictionary<string, EditText>(StringComparer.Ordinal);
        var content = new LinearLayout(this) { Orientation = Orientation.Vertical };
        content.SetPadding(Dp(20), Dp(18), Dp(20), Dp(18));
        var title = Text(Safe(form.Title), 20, "#F4C95D");
        title.SetTypeface(null, Android.Graphics.TypefaceStyle.Bold);
        content.AddView(title, MatchWidth());
        AddWithTopMargin(
            content,
            Text($"{Safe(runtime.Name)} / revision {runtime.Revision}", 13, "#B9AA8A"),
            6);
        foreach (var field in form.Fields)
        {
            var label = field.Required ? $"{field.Label} *" : field.Label;
            AddWithTopMargin(content, Text(Safe(label), 14, "#E9E1D0"), 12);
            var input = Input(Safe(field.Placeholder ?? string.Empty));
            input.ContentDescription = Safe(field.Label);
            input.InputType = field.InputKind == UiFormInputKind.PathToken
                ? InputTypes.ClassText | InputTypes.TextVariationVisiblePassword
                : InputTypes.ClassText;
            input.SetFilters([new InputFilterLengthFilter(field.MaxLength)]);
            fields.Add(field.Key, input);
            AddWithTopMargin(content, input, 4);
        }
        var validation = Text(string.Empty, 13, "#FF8A65");
        validation.AccessibilityLiveRegion = AccessibilityLiveRegion.Assertive;
        AddWithTopMargin(content, validation, 8);
        AddWithTopMargin(
            content,
            Text(
                "Values remain local until a validated submit event and explicit confirmation.",
                12,
                "#B9AA8A"),
            8);
        var buttons = new LinearLayout(this) { Orientation = Orientation.Horizontal };
        buttons.SetGravity(GravityFlags.End | GravityFlags.CenterVertical);
        var cancel = new Button(this) { Text = "Cancel" };
        var submit = new Button(this) { Text = Safe(form.SubmitLabel) };
        cancel.SetMinimumHeight(Dp(MobileLayoutPolicy.MinimumTouchTargetDp));
        submit.SetMinimumHeight(Dp(MobileLayoutPolicy.MinimumTouchTargetDp));
        buttons.AddView(cancel, WrapContent());
        var submitParameters = WrapContent();
        submitParameters.LeftMargin = Dp(8);
        buttons.AddView(submit, submitParameters);
        AddWithTopMargin(content, buttons, 12);

        var scroll = new ScrollView(this) { FillViewport = true };
        scroll.AddView(content, MatchWidth());
        var builder = new AlertDialog.Builder(this);
        builder.SetView(scroll);
        builder.SetCancelable(false);
        var dialog = builder.Create()
            ?? throw new InvalidOperationException("Android form dialog is unavailable");
        cancel.Click += (_, _) =>
        {
            completion.TrySetResult(null);
            dialog.Dismiss();
        };
        submit.Click += (_, _) =>
        {
            var values = fields
                .Where(entry => !string.IsNullOrEmpty(entry.Value.Text))
                .ToDictionary(
                    entry => entry.Key,
                    entry => entry.Value.Text!,
                    StringComparer.Ordinal);
            var feed = coordinator.State.Remote?.Feed;
            var resolution = feed is null
                ? new RemoteUiActionResolution(
                    null,
                    RemoteUiActionFailure.ActionUnavailable,
                    "The remote feed retired")
                : source.ResolveSubmission(node.Id, values, feed);
            if (!resolution.Accepted)
            {
                validation.Text = Safe(resolution.Reason ?? "Form values are invalid.");
                return;
            }
            completion.TrySetResult(values);
            dialog.Dismiss();
        };
        dialog.Show();
        dialog.SetCanceledOnTouchOutside(false);
        using var cancellation = lifetime.Token.Register(() => RunOnUiThread(() =>
        {
            completion.TrySetCanceled(lifetime.Token);
            dialog.Dismiss();
        }));
        return await completion.Task;
    }

    private async Task<bool> ShowConfirmationAsync(
        string title,
        string message,
        string positiveLabel)
    {
        var completion = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var content = new LinearLayout(this) { Orientation = Orientation.Vertical };
        content.SetPadding(Dp(20), Dp(18), Dp(20), Dp(18));
        var heading = Text(Safe(title), 20, "#F4C95D");
        heading.SetTypeface(null, Android.Graphics.TypefaceStyle.Bold);
        content.AddView(heading, MatchWidth());
        AddWithTopMargin(content, Text(Safe(message), 14, "#E9E1D0"), 10);
        var buttons = new LinearLayout(this) { Orientation = Orientation.Horizontal };
        buttons.SetGravity(GravityFlags.End | GravityFlags.CenterVertical);
        var cancel = new Button(this) { Text = "Cancel" };
        var confirm = new Button(this) { Text = Safe(positiveLabel) };
        cancel.SetMinimumHeight(Dp(MobileLayoutPolicy.MinimumTouchTargetDp));
        confirm.SetMinimumHeight(Dp(MobileLayoutPolicy.MinimumTouchTargetDp));
        buttons.AddView(cancel, WrapContent());
        var confirmParameters = WrapContent();
        confirmParameters.LeftMargin = Dp(8);
        buttons.AddView(confirm, confirmParameters);
        AddWithTopMargin(content, buttons, 14);
        var builder = new AlertDialog.Builder(this);
        builder.SetView(content);
        builder.SetCancelable(false);
        var dialog = builder.Create()
            ?? throw new InvalidOperationException("Android confirmation dialog is unavailable");
        cancel.Click += (_, _) =>
        {
            completion.TrySetResult(false);
            dialog.Dismiss();
        };
        confirm.Click += (_, _) =>
        {
            completion.TrySetResult(true);
            dialog.Dismiss();
        };
        dialog.Show();
        dialog.SetCanceledOnTouchOutside(false);
        using var cancellation = lifetime.Token.Register(() => RunOnUiThread(() =>
        {
            completion.TrySetCanceled(lifetime.Token);
            dialog.Dismiss();
        }));
        return await completion.Task;
    }

    private void ReturnToFleet()
    {
        activeWorkspaceRuntimeId = null;
        activeDocument = null;
        operationStatus = null;
        operationFailed = false;
        Render(coordinator.State);
    }

    private void SetOperationStatus(string message, bool failed)
    {
        operationStatus = Safe(message);
        operationFailed = failed;
        Render(coordinator.State);
    }

    private View EmptyProjection(string message)
    {
        var empty = Text(message, 14, "#B9AA8A");
        empty.Gravity = GravityFlags.CenterVertical;
        empty.SetPadding(Dp(14), Dp(12), Dp(14), Dp(12));
        empty.SetMinimumHeight(Dp(MobileLayoutPolicy.MinimumTouchTargetDp));
        empty.Background = PanelBackground("#17140F");
        return empty;
    }

    private void AddProjectedView(LinearLayout parent, View child, int topDp)
    {
        var parameters = MatchWidth();
        parameters.TopMargin = Dp(topDp);
        parent.AddView(child, parameters);
    }

    private static bool ActionEnabled(
        ActionKind? kind,
        RemoteMutationAvailability availability) => kind switch
        {
            ActionKind.RuntimeInspect => availability.InspectEnabled,
            ActionKind.RuntimeRefresh
                or ActionKind.RuntimeCapabilitiesRefresh
                or ActionKind.RuntimeDeploy => availability.MutationsEnabled,
            _ => false,
        };

    private static string? ActionUnavailableReason(
        ActionKind? kind,
        RemoteMutationAvailability availability) => kind switch
        {
            ActionKind.RuntimeInspect => availability.InspectUnavailableReason,
            ActionKind.RuntimeRefresh
                or ActionKind.RuntimeCapabilitiesRefresh
                or ActionKind.RuntimeDeploy => availability.MutationUnavailableReason,
            _ => "Unsupported mobile action",
        };

    private static string LogColor(string? value) => value?.StartsWith(
        "[ERROR]",
        StringComparison.Ordinal) == true
        ? "#FF8A65"
        : "#CFC4AE";

    private static string ConfirmationTitle(ActionKind kind) => kind switch
    {
        ActionKind.RuntimeRefresh => "Confirm runtime refresh",
        ActionKind.RuntimeCapabilitiesRefresh => "Confirm capability discovery",
        ActionKind.RuntimeDeploy => "Confirm remote deployment",
        _ => "Confirm remote change",
    };

    private static string ConfirmationAction(ActionKind kind) => kind switch
    {
        ActionKind.RuntimeRefresh => "Refresh",
        ActionKind.RuntimeCapabilitiesRefresh => "Discover",
        ActionKind.RuntimeDeploy => "Deploy",
        _ => "Continue",
    };

    private static string ConfirmationMessage(RemoteUiActionIntent intent)
    {
        var target = intent.Target is null ? string.Empty : $" / target {Safe(intent.Target)}";
        var pipeline = intent.PipelineKind is null
            ? string.Empty
            : $" / pipeline {Safe(intent.PipelineKind)}";
        return $"{Safe(intent.Runtime.Name)} / expected revision {intent.Runtime.Revision}{pipeline}{target}. This command is not retried automatically.";
    }

    private static string MutationProgress(RemoteUiActionIntent intent) =>
        $"{MutationLabel(intent.Kind)} for {Safe(intent.Runtime.Name)} at revision {intent.Runtime.Revision}...";

    private static string MutationLabel(ActionKind kind) => kind switch
    {
        ActionKind.RuntimeRefresh => "Runtime refresh",
        ActionKind.RuntimeCapabilitiesRefresh => "Capability discovery",
        ActionKind.RuntimeDeploy => "Deployment",
        _ => "Remote change",
    };

    private static string WorkspaceFailure(Exception error) => error switch
    {
        RemoteQueryException query => $"Workspace query rejected ({Safe(query.Code)}).",
        MobileRemoteGenerationRetiredException =>
            "Workspace request retired after the active connection changed.",
        InvalidOperationException local => Safe(local.Message),
        ArgumentException local => Safe(local.Message),
        InvalidDataException => "Workspace response failed validation.",
        System.Net.Http.HttpRequestException => "Workspace network request failed safely.",
        _ => "Workspace request failed safely.",
    };

    private static string MutationFailure(Exception error) => error switch
    {
        MobileRemoteMutationException failure => Safe(failure.Message),
        InvalidOperationException local => Safe(local.Message),
        ArgumentException local => Safe(local.Message),
        _ => "Remote change failed safely.",
    };

    private static string ConnectionFailure(Exception error) => error switch
    {
        InvalidDataException local => Safe(local.Message),
        ArgumentException local => Safe(local.Message),
        System.Security.Cryptography.CryptographicException =>
            "Credential or certificate validation failed.",
        System.Net.Http.HttpRequestException => "Network connection failed safely.",
        _ => "Connection failed safely.",
    };

    private void ShowError(Exception error) => RunOnUiThread(() =>
    {
        statusText.Text = $"Connection blocked: {ConnectionFailure(error)}";
        statusText.SetTextColor(Color.ParseColor("#FF8A65"));
    });

    private void SetConnectEnabled(bool enabled)
    {
        connectButton.Enabled = enabled;
        connectButton.BackgroundTintList = Android.Content.Res.ColorStateList.ValueOf(
            Color.ParseColor(enabled ? "#FFB229" : "#5A5142"));
        connectButton.SetTextColor(Color.ParseColor(enabled ? "#11100D" : "#D3C8B2"));
    }

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
        input.SetPadding(Dp(12), Dp(8), Dp(12), Dp(8));
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
        text.SetLineSpacing(0, 1.08f);
        return text;
    }

    private LinearLayout Section()
    {
        var section = new LinearLayout(this)
        {
            Orientation = Orientation.Vertical,
            Background = PanelBackground("#17140F"),
        };
        section.SetPadding(Dp(16), Dp(16), Dp(16), Dp(16));
        return section;
    }

    private GradientDrawable PanelBackground(string color)
    {
        var background = new GradientDrawable();
        background.SetColor(Color.ParseColor(color));
        background.SetCornerRadius(Dp(12));
        background.SetStroke(Dp(1), Color.ParseColor("#3A3021"));
        return background;
    }

    private void AddWithTopMargin(LinearLayout parent, View child, int topDp)
    {
        var parameters = MatchWidth();
        parameters.TopMargin = Dp(topDp);
        parent.AddView(child, parameters);
    }

    private LinearLayout.LayoutParams WeightedSection(
        float weight,
        int leftMargin = 0,
        int rightMargin = 0) => new(
            0,
            ViewGroup.LayoutParams.WrapContent,
            weight)
        {
            LeftMargin = Dp(leftMargin),
            RightMargin = Dp(rightMargin),
        };

    private static LinearLayout.LayoutParams MatchWidth() => new(
        ViewGroup.LayoutParams.MatchParent,
        ViewGroup.LayoutParams.WrapContent);

    private static LinearLayout.LayoutParams WrapContent() => new(
        ViewGroup.LayoutParams.WrapContent,
        ViewGroup.LayoutParams.WrapContent);

    private int Dp(int value) => (int)(value * Resources!.DisplayMetrics!.Density + 0.5f);

    private double PxToDp(int value) => value / Resources!.DisplayMetrics!.Density;

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());

    private sealed class AdaptiveRootLayout(
        Context context,
        Action<WindowInsets> insetsChanged) : LinearLayout(context)
    {
        public override WindowInsets? OnApplyWindowInsets(WindowInsets? insets)
        {
            if (insets is not null)
            {
                insetsChanged(insets);
            }
            return insets;
        }
    }
}
