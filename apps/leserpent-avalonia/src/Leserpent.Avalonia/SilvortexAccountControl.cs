using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class SilvortexAccountControl : Border, IDisposable
{
    private readonly DesktopLocalization localization;
    private readonly SilvortexAccountSession session;
    private readonly TextBlock labelText = new()
    {
        Foreground = LeserpentTheme.Accent,
        FontSize = 10,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 1.3,
        VerticalAlignment = VerticalAlignment.Center,
    };
    private readonly TextBlock identityText = new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextTrimming = TextTrimming.CharacterEllipsis,
    };
    private readonly TextBlock statusText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 11,
        TextWrapping = TextWrapping.Wrap,
        MaxWidth = 270,
    };
    private readonly Button actionButton = new()
    {
        Padding = new Thickness(12, 7),
        MinWidth = 88,
        HorizontalAlignment = HorizontalAlignment.Right,
    };
    private SilvortexAccountSnapshot snapshot;
    private bool disposed;

    public SilvortexAccountControl(
        SilvortexAccountSession session,
        DesktopLocalization localization)
    {
        this.session = session;
        this.localization = localization;
        snapshot = session.Snapshot;
        Background = Brush.Parse("#17150F");
        BorderBrush = LeserpentTheme.PanelBorder;
        BorderThickness = new Thickness(1);
        CornerRadius = new CornerRadius(10);
        Padding = new Thickness(14, 11);
        Width = 300;
        MinWidth = 260;
        MaxWidth = 300;
        VerticalAlignment = VerticalAlignment.Top;

        AutomationProperties.SetAutomationId(statusText, "hub-silvortex-status");
        AutomationProperties.SetAutomationId(actionButton, "hub-silvortex-action");
        var marker = new Border
        {
            Background = Brush.Parse("#34230E"),
            CornerRadius = new CornerRadius(12),
            Width = 24,
            Height = 24,
            Child = new TextBlock
            {
                Text = "S",
                Foreground = LeserpentTheme.Primary,
                FontSize = 11,
                FontWeight = FontWeight.Bold,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };
        var labelRow = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { marker, labelText },
        };
        var body = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
            ColumnSpacing = 10,
            Children =
            {
                new StackPanel
                {
                    Spacing = 3,
                    Children = { identityText, statusText },
                },
                actionButton,
            },
        };
        Grid.SetColumn(actionButton, 1);
        Child = new StackPanel
        {
            Spacing = 9,
            Children = { labelRow, body },
        };

        actionButton.Click += OnAction;
        session.SnapshotChanged += OnSnapshotChanged;
        localization.Changed += OnLocalizationChanged;
        ApplyLocalization();
        session.BeginRestore();
    }

    public IReadOnlyList<Control> AuditedControls => [statusText, actionButton];

    public void Dispose()
    {
        if (disposed)
        {
            return;
        }
        disposed = true;
        actionButton.Click -= OnAction;
        session.SnapshotChanged -= OnSnapshotChanged;
        localization.Changed -= OnLocalizationChanged;
    }

    public void VerifyLayoutEnvelope()
    {
        Measure(new Size(Width, 360));
        var desired = DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 360)
        {
            throw new InvalidDataException(
                "Team Silvortex account controls exceeded their layout envelope");
        }
    }

    public void ProbeLocalizedPresentation(
        string expectedLabel,
        string expectedIdentity,
        string expectedAction,
        string expectedStatus)
    {
        if (labelText.Text != expectedLabel
            || identityText.Text != expectedIdentity
            || actionButton.Content as string != expectedAction
            || statusText.Text != expectedStatus
            || AutomationProperties.GetName(actionButton) != Text("a11y.sign_in")
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "Team Silvortex localized account presentation drifted");
        }
    }

    private void OnAction(object? sender, Avalonia.Interactivity.RoutedEventArgs eventArgs)
    {
        _ = session.Snapshot.IsSignedIn
            ? session.SignOutAsync()
            : session.SignInAsync();
    }

    private void OnSnapshotChanged(SilvortexAccountSnapshot value)
    {
        if (disposed)
        {
            return;
        }
        Dispatcher.UIThread.Post(() =>
        {
            if (!disposed)
            {
                snapshot = value;
                Apply(value);
            }
        });
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs)
    {
        if (!disposed)
        {
            ApplyLocalization();
        }
    }

    private void ApplyLocalization()
    {
        FlowDirection = localization.FlowDirection;
        labelText.Text = Text("label");
        AutomationProperties.SetHelpText(actionButton, Text("a11y.help"));
        Apply(snapshot);
    }

    private void Apply(SilvortexAccountSnapshot value)
    {
        snapshot = value;
        identityText.Text = value.Phase switch
        {
            SilvortexAccountPhase.Disabled => Text("identity.disabled"),
            SilvortexAccountPhase.SignedOut => Text("identity.signed_out"),
            SilvortexAccountPhase.Working => Text("identity.working"),
            SilvortexAccountPhase.SignedIn => value.DisplayName
                ?? value.Email
                ?? value.Subject
                ?? Text("identity.authenticated"),
            SilvortexAccountPhase.Error => Text("identity.error"),
            _ => Text("identity.unavailable"),
        };
        statusText.Text = value.IsSignedIn
            ? value.Email ?? value.Subject ?? Status(value)
            : Status(value);
        statusText.Foreground = value.Phase == SilvortexAccountPhase.Error
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Muted;
        actionButton.Content = value.Phase switch
        {
            SilvortexAccountPhase.Disabled => Text("action.disabled"),
            SilvortexAccountPhase.Working => Text("action.working"),
            SilvortexAccountPhase.SignedIn => Text("action.signed_in"),
            SilvortexAccountPhase.Error => Text("action.error"),
            _ => Text("action.signed_out"),
        };
        actionButton.IsEnabled = value.Phase is
            SilvortexAccountPhase.SignedOut
            or SilvortexAccountPhase.SignedIn
            or SilvortexAccountPhase.Error;
        actionButton.Background = value.IsSignedIn
            ? LeserpentTheme.PanelBorder
            : LeserpentTheme.Accent;
        actionButton.Foreground = value.IsSignedIn
            ? LeserpentTheme.Body
            : Brushes.Black;
        AutomationProperties.SetName(
            actionButton,
            value.IsSignedIn
                ? Text("a11y.sign_out")
                : Text("a11y.sign_in"));
        AutomationProperties.SetName(
            statusText,
            Format("a11y.status", identityText.Text ?? string.Empty, statusText.Text ?? string.Empty));
    }

    private string Status(SilvortexAccountSnapshot value) => value.Status switch
    {
        SilvortexAccountStatus.ConfigurationInvalid =>
            Format("status.configuration_invalid", StatusDetail(value)),
        SilvortexAccountStatus.OverrideRefused => Text("status.override_refused"),
        SilvortexAccountStatus.OptionalBundle => Text("status.optional_bundle"),
        SilvortexAccountStatus.BundleReady => Text("status.bundle_ready"),
        SilvortexAccountStatus.OptionalBuild => Text("status.optional_build"),
        SilvortexAccountStatus.MissingIssuer => Format(
            "status.missing_issuer",
            SilvortexAccountOptions.IssuerEnvironmentVariable),
        SilvortexAccountStatus.DevelopmentReady => Text("status.development_ready"),
        SilvortexAccountStatus.VerificationDisabled => Text("status.verification_disabled"),
        SilvortexAccountStatus.ProofReady => Text("status.proof_ready"),
        SilvortexAccountStatus.OpeningBrowser => Text("status.opening_browser"),
        SilvortexAccountStatus.AwaitingCallback => Text("status.awaiting_callback"),
        SilvortexAccountStatus.SignInFailed =>
            Format("status.sign_in_failed", StatusDetail(value)),
        SilvortexAccountStatus.SigningOut => Text("status.signing_out"),
        SilvortexAccountStatus.SignedOut => Text("status.signed_out"),
        SilvortexAccountStatus.SignOutFailed =>
            Format("status.sign_out_failed", StatusDetail(value)),
        SilvortexAccountStatus.SignInRequired => Text("status.sign_in_required"),
        SilvortexAccountStatus.Restoring => Text("status.restoring"),
        SilvortexAccountStatus.RestoreFailed =>
            Format("status.restore_failed", StatusDetail(value)),
        SilvortexAccountStatus.Authenticated => Text("status.authenticated"),
        SilvortexAccountStatus.Raw => Format("status.raw", Safe(value.Message)),
        _ => throw new InvalidDataException(
            "unsupported Team Silvortex account presentation status"),
    };

    private static string StatusDetail(SilvortexAccountSnapshot value) =>
        Safe(value.StatusDetail ?? value.Message);

    private string Text(string key) => DesktopAccountCatalogs.Resolve(localization, key);

    private string Format(string key, params object[] values) =>
        DesktopAccountCatalogs.Format(localization, key, values);

    private static string Safe(string value) =>
        new(value.Where(character => !char.IsControl(character)).Take(512).ToArray());
}
