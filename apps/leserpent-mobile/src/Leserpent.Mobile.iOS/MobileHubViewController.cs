using Foundation;
using UIKit;

public sealed class MobileHubViewController : UIViewController, IAsyncDisposable
{
    private readonly IosConnectionProfileStore profileStore;
    private readonly MobileApplicationCoordinator coordinator;
    private readonly CancellationTokenSource lifetime = new();
    private UIScrollView scroll = null!;
    private UIStackView content = null!;
    private UIStackView body = null!;
    private UIStackView connectionFields = null!;
    private UILabel connectionSummary = null!;
    private UILabel status = null!;
    private UITextField endpointInput = null!;
    private UITextView certificateInput = null!;
    private UITextField tokenInput = null!;
    private UIButton editConnectionButton = null!;
    private UIButton connectButton = null!;
    private UIButton backToFleetButton = null!;
    private UIStackView connectionHeader = null!;
    private UIStackView documentHeader = null!;
    private IosUiDocumentView documentView = null!;
    private NSLayoutConstraint contentMaximumWidth = null!;
    private readonly MobileNativeRenderGate renderGate = new();
    private MobileLayoutPlan? appliedLayout;
    private MobileUiDocumentBinding? activeDocument;
    private string? activeWorkspaceRuntimeId;
    private string? operationStatus;
    private bool operationFailed;
    private bool operationBusy;
    private bool connectionExpanded;
    private bool disposed;

    public MobileHubViewController(
        IosConnectionProfileStore profileStore,
        IMobileCredentialVault vault)
    {
        this.profileStore = profileStore;
        coordinator = new MobileApplicationCoordinator(vault);
        coordinator.StateChanged += OnStateChanged;
    }

    public override void ViewDidLoad()
    {
        base.ViewDidLoad();
        BuildInterface();
        var profile = profileStore.Load();
        endpointInput.Text = profile?.Endpoint ?? string.Empty;
        UpdateConnectionSummary(profile?.Endpoint);
        var expandConnection = profile is null;
#if DEBUG
        expandConnection &= !NSProcessInfo.ProcessInfo.Arguments.Contains(
            "--leserpent-ui-proof",
            StringComparer.Ordinal);
#endif
        SetConnectionExpanded(expandConnection);
        Render(coordinator.State);
    }

    public override void ViewDidLayoutSubviews()
    {
        base.ViewDidLayoutSubviews();
        ApplyLayout();
    }

    public async Task EnterForegroundAsync()
    {
        if (disposed)
        {
            return;
        }
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
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowSafeError(error);
        }
    }

    public async Task EnterBackgroundAsync()
    {
        if (disposed)
        {
            return;
        }
        try
        {
            await coordinator.EnterBackgroundAsync();
        }
        catch (ObjectDisposedException) when (disposed)
        {
        }
        catch (Exception error)
        {
            ShowSafeError(error);
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (disposed)
        {
            return;
        }
        disposed = true;
        lifetime.Cancel();
        coordinator.StateChanged -= OnStateChanged;
        await coordinator.DisposeAsync();
        lifetime.Dispose();
    }

    private void BuildInterface()
    {
        View!.BackgroundColor = Theme.Background;
        scroll = new UIScrollView
        {
            AlwaysBounceVertical = true,
            KeyboardDismissMode = UIScrollViewKeyboardDismissMode.Interactive,
            TranslatesAutoresizingMaskIntoConstraints = false,
        };
        content = new UIStackView
        {
            Axis = UILayoutConstraintAxis.Vertical,
            Spacing = 20,
            LayoutMarginsRelativeArrangement = true,
            TranslatesAutoresizingMaskIntoConstraints = false,
        };
        View.AddSubview(scroll);
        scroll.AddSubview(content);
        var width = content.WidthAnchor.ConstraintEqualTo(scroll.FrameLayoutGuide.WidthAnchor);
        width.Priority = 750;
        contentMaximumWidth = content.WidthAnchor.ConstraintLessThanOrEqualTo(1120);
        NSLayoutConstraint.ActivateConstraints([
            scroll.LeadingAnchor.ConstraintEqualTo(View.LeadingAnchor),
            scroll.TrailingAnchor.ConstraintEqualTo(View.TrailingAnchor),
            scroll.TopAnchor.ConstraintEqualTo(View.TopAnchor),
            scroll.BottomAnchor.ConstraintEqualTo(View.KeyboardLayoutGuide.TopAnchor),
            content.TopAnchor.ConstraintEqualTo(scroll.ContentLayoutGuide.TopAnchor),
            content.BottomAnchor.ConstraintEqualTo(scroll.ContentLayoutGuide.BottomAnchor),
            content.CenterXAnchor.ConstraintEqualTo(scroll.FrameLayoutGuide.CenterXAnchor),
            width,
            contentMaximumWidth,
        ]);

        content.AddArrangedSubview(Label(
            "LESERPENT",
            UIFontTextStyle.Subheadline,
            Theme.Orange,
            bold: true));
        content.AddArrangedSubview(Label(
            "Remote control, without semantic forks",
            UIFontTextStyle.LargeTitle,
            Theme.Gold,
            bold: true));
        content.AddArrangedSubview(Label(
            "One secure authority at a time. Native controls consume the same semantic document as desktop and web.",
            UIFontTextStyle.Body,
            Theme.Muted));

        var connectionHeaderText = new UIStackView
        {
            Axis = UILayoutConstraintAxis.Vertical,
            Spacing = 4,
        };
        connectionHeaderText.AddArrangedSubview(Label(
            "Authority",
            UIFontTextStyle.Title2,
            Theme.Gold,
            bold: true));
        connectionSummary = Label("Not configured", UIFontTextStyle.Body, Theme.Muted);
        connectionHeaderText.AddArrangedSubview(connectionSummary);
        editConnectionButton = Button("Edit connection", primary: false);
        editConnectionButton.AccessibilityHint =
            "Shows or hides authority credentials and trust setup.";
        editConnectionButton.TouchUpInside += (_, _) =>
            SetConnectionExpanded(!connectionExpanded);
        connectionHeader = new UIStackView([connectionHeaderText, editConnectionButton])
        {
            Axis = UILayoutConstraintAxis.Horizontal,
            Alignment = UIStackViewAlignment.Center,
            Spacing = 12,
        };

        endpointInput = TextField("https://control.example:9443");
        endpointInput.KeyboardType = UIKeyboardType.Url;
        endpointInput.AutocapitalizationType = UITextAutocapitalizationType.None;
        endpointInput.AutocorrectionType = UITextAutocorrectionType.No;
        certificateInput = new UITextView
        {
            BackgroundColor = Theme.Input,
            TextColor = Theme.Text,
            Font = UIFont.GetPreferredFontForTextStyle(UIFontTextStyle.Body)!,
            AdjustsFontForContentSizeCategory = true,
            Layer = { CornerRadius = 10 },
            AccessibilityLabel = "CA certificate PEM",
            TranslatesAutoresizingMaskIntoConstraints = false,
        };
        certificateInput.HeightAnchor.ConstraintGreaterThanOrEqualTo(144).Active = true;
        tokenInput = TextField("Keystore protected");
        tokenInput.SecureTextEntry = true;
        tokenInput.AutocapitalizationType = UITextAutocapitalizationType.None;
        tokenInput.AutocorrectionType = UITextAutocorrectionType.No;
        connectButton = Button("Save and connect", primary: true);
        connectButton.AccessibilityHint =
            "Stores the token in Keychain and connects to the validated HTTPS authority.";
        connectButton.TouchUpInside += async (_, _) => await ConfigureAndConnectAsync();
        connectionFields = new UIStackView
        {
            Axis = UILayoutConstraintAxis.Vertical,
            Spacing = 10,
        };
        connectionFields.AddArrangedSubview(Field("HTTPS authority", endpointInput));
        connectionFields.AddArrangedSubview(Field("CA certificate PEM", certificateInput));
        connectionFields.AddArrangedSubview(Field("Endpoint-scoped token", tokenInput));
        connectionFields.AddArrangedSubview(connectButton);
        var connectionCard = Card([connectionHeader, connectionFields]);

        status = Label("Not configured", UIFontTextStyle.Body, Theme.Muted);
        status.AccessibilityTraits = UIAccessibilityTrait.UpdatesFrequently;
        backToFleetButton = Button("Back to runtimes", primary: false);
        backToFleetButton.TouchUpInside += (_, _) =>
        {
            activeWorkspaceRuntimeId = null;
            activeDocument = null;
            operationStatus = null;
            operationFailed = false;
            Render(coordinator.State);
        };
        documentView = new IosUiDocumentView(InvokeActionAsync);
        documentHeader = new UIStackView([status, backToFleetButton])
        {
            Axis = UILayoutConstraintAxis.Horizontal,
            Alignment = UIStackViewAlignment.Center,
            Spacing = 12,
        };
        var documentCard = Card([documentHeader, documentView]);

        body = new UIStackView([connectionCard, documentCard])
        {
            Axis = UILayoutConstraintAxis.Vertical,
            Distribution = UIStackViewDistribution.Fill,
            Alignment = UIStackViewAlignment.Fill,
            Spacing = 20,
        };
        content.AddArrangedSubview(body);
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
            operationFailed = false;
            await coordinator.EnterForegroundAsync(lifetime.Token);
            UpdateConnectionSummary(profile.Endpoint);
            SetConnectionExpanded(false);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowSafeError(error);
        }
        finally
        {
            SetConnectEnabled(true);
        }
    }

    private void ApplyLayout()
    {
        var view = View!;
        if (view.Bounds.Width <= 0 || view.Bounds.Height <= 0)
        {
            return;
        }
        var safe = view.SafeAreaInsets;
        var preferredBody = UIFont.GetPreferredFontForTextStyle(UIFontTextStyle.Body)!;
        var fontScale = Math.Max(1, preferredBody.PointSize / 17.0);
        var plan = MobileLayoutPolicy.Resolve(
            view.Bounds.Width,
            view.Bounds.Height,
            fontScale,
            new MobileSafeAreaInsets(safe.Left, safe.Top, safe.Right, safe.Bottom));
        if (appliedLayout == plan)
        {
            return;
        }
        var previousColumns = appliedLayout?.RuntimeColumns;
        appliedLayout = plan;
        content.LayoutMargins = new UIEdgeInsets(
            plan.ContentInsets.Top,
            plan.ContentInsets.Left,
            plan.ContentInsets.Bottom,
            plan.ContentInsets.Right);
        content.Spacing = plan.SectionSpacingDp;
        contentMaximumWidth.Constant = plan.ContentMaxWidthDp == 0
            ? view.Bounds.Width
            : plan.ContentMaxWidthDp;
        body.Axis = plan.TwoPane
            ? UILayoutConstraintAxis.Horizontal
            : UILayoutConstraintAxis.Vertical;
        body.Distribution = plan.TwoPane
            ? UIStackViewDistribution.FillEqually
            : UIStackViewDistribution.Fill;
        var compact = plan.WidthClass == MobileWidthClass.Compact;
        connectionHeader.Axis = compact
            ? UILayoutConstraintAxis.Vertical
            : UILayoutConstraintAxis.Horizontal;
        connectionHeader.Alignment = compact
            ? UIStackViewAlignment.Fill
            : UIStackViewAlignment.Center;
        documentHeader.Axis = compact
            ? UILayoutConstraintAxis.Vertical
            : UILayoutConstraintAxis.Horizontal;
        documentHeader.Alignment = compact
            ? UIStackViewAlignment.Fill
            : UIStackViewAlignment.Center;
        if (previousColumns != plan.RuntimeColumns)
        {
            Render(coordinator.State);
        }
    }

    private void SetConnectionExpanded(bool expanded)
    {
        connectionExpanded = expanded;
        connectionFields.Hidden = !expanded;
        editConnectionButton.SetTitle(
            expanded ? "Hide setup" : "Edit connection",
            UIControlState.Normal);
        editConnectionButton.AccessibilityLabel = expanded
            ? "Hide authority connection setup"
            : "Edit the saved authority connection";
    }

    private void UpdateConnectionSummary(string? endpoint) =>
        connectionSummary.Text = string.IsNullOrWhiteSpace(endpoint)
            ? "Not configured"
            : Safe(endpoint);

    private void OnStateChanged(MobileApplicationSnapshot snapshot) =>
        InvokeOnMainThread(() => Render(snapshot));

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
            status.Text = Safe(
                operationStatus
                ?? snapshot.Error
                ?? documentStatus
                ?? feed.Detail
                ?? snapshot.Phase.ToString());
            status.TextColor = operationFailed || snapshot.Error is not null
                ? Theme.Error
                : Theme.Muted;
            backToFleetButton.Hidden = activeWorkspaceRuntimeId is null;
            var availability = coordinator.MutationAvailability;
            var runtimeColumns = appliedLayout?.RuntimeColumns ?? 1;
            if (renderGate.ShouldRender(
                    document,
                    availability,
                    operationBusy,
                    runtimeColumns))
            {
                documentView.Mount(
                    document,
                    availability,
                    operationBusy,
                    runtimeColumns);
            }
        }
        catch (InvalidDataException)
        {
            status.Text = "UI document rejected safely.";
            status.TextColor = Theme.Error;
            renderGate.Invalidate();
            documentView.MountFailure("Remote projection is unavailable.");
        }
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
            SetOperationStatus(
                $"Remote action blocked: {Safe(activation.Reason ?? "unavailable")}",
                true);
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
                SetOperationStatus(
                    "Deployment blocked: the shared form is unavailable.",
                    true);
                return;
            }
            var values = await ShowParameterizedFormAsync(form);
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
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            operationStatus = "Workspace request stopped when the app left foreground.";
            operationFailed = false;
        }
        catch (Exception error) when (error is not OperationCanceledException)
        {
            operationStatus = WorkspaceFailure(error);
            operationFailed = true;
        }
        finally
        {
            operationBusy = false;
            if (!disposed)
            {
                Render(coordinator.State);
            }
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
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            operationStatus = "Remote change stopped when the app left foreground.";
            operationFailed = false;
        }
        catch (Exception error) when (error is not OperationCanceledException)
        {
            operationStatus = MutationFailure(error);
            operationFailed = true;
        }
        finally
        {
            operationBusy = false;
            if (!disposed)
            {
                Render(coordinator.State);
            }
        }
    }

    private async Task<IReadOnlyDictionary<string, string>?> ShowParameterizedFormAsync(
        MobileUiFormBinding form)
    {
        var completion = new TaskCompletionSource<IReadOnlyDictionary<string, string>?>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var alert = UIAlertController.Create(
            Safe(form.Title),
            "Values remain local until a validated submit event and explicit confirmation.",
            UIAlertControllerStyle.Alert);
        var fields = new List<UITextField>();
        foreach (var field in form.Fields)
        {
            alert.AddTextField(input =>
            {
                input.Placeholder = Safe(field.Placeholder ?? field.Label);
                input.AccessibilityLabel = Safe(field.Label);
                input.AutocapitalizationType = UITextAutocapitalizationType.None;
                input.AutocorrectionType = UITextAutocorrectionType.No;
            });
            fields.Add(alert.TextFields!.Last());
        }
        alert.AddAction(UIAlertAction.Create(
            "Cancel",
            UIAlertActionStyle.Cancel,
            _ => completion.TrySetResult(null)));
        alert.AddAction(UIAlertAction.Create(
            Safe(form.SubmitLabel),
            UIAlertActionStyle.Default,
            _ => completion.TrySetResult(form.Fields
                .Select((field, index) => (field.Key, Value: fields[index].Text ?? string.Empty))
                .ToDictionary(value => value.Key, value => value.Value, StringComparer.Ordinal))));
        PresentViewController(alert, true, null);
        return await completion.Task;
    }

    private async Task<bool> ShowConfirmationAsync(
        string title,
        string message,
        string action)
    {
        var completion = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var alert = UIAlertController.Create(title, message, UIAlertControllerStyle.Alert);
        alert.AddAction(UIAlertAction.Create(
            "Cancel",
            UIAlertActionStyle.Cancel,
            _ => completion.TrySetResult(false)));
        alert.AddAction(UIAlertAction.Create(
            action,
            UIAlertActionStyle.Default,
            _ => completion.TrySetResult(true)));
        PresentViewController(alert, true, null);
        return await completion.Task;
    }

    private void SetOperationStatus(string value, bool failed)
    {
        operationStatus = value;
        operationFailed = failed;
        Render(coordinator.State);
    }

    private void ShowSafeError(Exception error) => InvokeOnMainThread(() =>
    {
        operationStatus = $"Connection blocked: {ConnectionFailure(error)}";
        operationFailed = true;
        Render(coordinator.State);
    });

    private void SetConnectEnabled(bool enabled)
    {
        connectButton.Enabled = enabled;
        connectButton.BackgroundColor = enabled ? Theme.Orange : Theme.Disabled;
        connectButton.SetTitleColor(enabled ? Theme.Background : Theme.Muted, UIControlState.Normal);
    }

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

    private static UIStackView Card(IReadOnlyList<UIView> children)
    {
        var card = new UIStackView(children.ToArray())
        {
            Axis = UILayoutConstraintAxis.Vertical,
            Spacing = 14,
            LayoutMarginsRelativeArrangement = true,
            LayoutMargins = new UIEdgeInsets(18, 18, 18, 18),
            BackgroundColor = Theme.Panel,
        };
        card.Layer.CornerRadius = 16;
        return card;
    }

    private static UIStackView Field(string title, UIView input) => new([
        Label(title, UIFontTextStyle.Subheadline, Theme.Text),
        input,
    ])
    {
        Axis = UILayoutConstraintAxis.Vertical,
        Spacing = 6,
    };

    private static UILabel Label(
        string value,
        UIFontTextStyle style,
        UIColor color,
        bool bold = false)
    {
        var descriptor = UIFontDescriptor.GetPreferredDescriptorForTextStyle(style);
        var font = UIFont.FromDescriptor(
            bold
                ? descriptor.CreateWithTraits(UIFontDescriptorSymbolicTraits.Bold)
                : descriptor,
            0)!;
        return new UILabel
        {
            Text = value,
            TextColor = color,
            Font = font,
            Lines = 0,
            AdjustsFontForContentSizeCategory = true,
        };
    }

    private static UITextField TextField(string placeholder) => new()
    {
        Placeholder = placeholder,
        BackgroundColor = Theme.Input,
        TextColor = Theme.Text,
        Font = UIFont.GetPreferredFontForTextStyle(UIFontTextStyle.Body)!,
        AdjustsFontForContentSizeCategory = true,
        BorderStyle = UITextBorderStyle.RoundedRect,
        ClearButtonMode = UITextFieldViewMode.WhileEditing,
    };

    private static UIButton Button(string title, bool primary)
    {
        var button = new UIButton(UIButtonType.System);
        button.SetTitle(title, UIControlState.Normal);
        button.SetTitleColor(primary ? Theme.Background : Theme.Text, UIControlState.Normal);
        button.BackgroundColor = primary ? Theme.Orange : Theme.SecondaryButton;
        button.TitleLabel!.Font = UIFont.GetPreferredFontForTextStyle(UIFontTextStyle.Headline)!;
        button.TitleLabel.AdjustsFontForContentSizeCategory = true;
        button.TitleLabel.Lines = 0;
        button.TitleLabel.LineBreakMode = UILineBreakMode.WordWrap;
        button.TitleLabel.TextAlignment = UITextAlignment.Center;
        button.Layer.CornerRadius = 12;
        button.HeightAnchor.ConstraintGreaterThanOrEqualTo(
            MobileLayoutPolicy.MinimumTouchTargetDp).Active = true;
        return button;
    }

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());

    private static class Theme
    {
        public static UIColor Background { get; } = UIColor.FromRGB(17, 16, 13);
        public static UIColor Panel { get; } = UIColor.FromRGB(28, 25, 19);
        public static UIColor Input { get; } = UIColor.FromRGB(32, 28, 21);
        public static UIColor SecondaryButton { get; } = UIColor.FromRGB(74, 65, 51);
        public static UIColor Disabled { get; } = UIColor.FromRGB(90, 81, 66);
        public static UIColor Orange { get; } = UIColor.FromRGB(255, 178, 41);
        public static UIColor Gold { get; } = UIColor.FromRGB(244, 201, 93);
        public static UIColor Text { get; } = UIColor.FromRGB(233, 225, 208);
        public static UIColor Muted { get; } = UIColor.FromRGB(185, 170, 138);
        public static UIColor Error { get; } = UIColor.FromRGB(255, 138, 101);
    }
}
