using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class SilvortexAccountControl : Border, IDisposable
{
    private readonly SilvortexAccountSession session;
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
    private bool disposed;

    public SilvortexAccountControl(SilvortexAccountSession session)
    {
        this.session = session;
        Background = Brush.Parse("#17150F");
        BorderBrush = LeserpentTheme.PanelBorder;
        BorderThickness = new Thickness(1);
        CornerRadius = new CornerRadius(10);
        Padding = new Thickness(14, 11);
        Width = 300;
        VerticalAlignment = VerticalAlignment.Top;

        AutomationProperties.SetAutomationId(statusText, "hub-silvortex-status");
        AutomationProperties.SetName(statusText, "Team Silvortex account status");
        AutomationProperties.SetAutomationId(actionButton, "hub-silvortex-action");
        AutomationProperties.SetName(actionButton, "Sign in to Team Silvortex");
        AutomationProperties.SetHelpText(
            actionButton,
            "Uses the system browser and a protected local PKCE callback. Daemon credentials remain separate.");

        var label = new TextBlock
        {
            Text = "TEAM SILVORTEX",
            Foreground = LeserpentTheme.Accent,
            FontSize = 10,
            FontWeight = FontWeight.Bold,
            LetterSpacing = 1.3,
            VerticalAlignment = VerticalAlignment.Center,
        };
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
            Children = { marker, label },
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
        Apply(session.Snapshot);
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
                Apply(value);
            }
        });
    }

    private void Apply(SilvortexAccountSnapshot value)
    {
        identityText.Text = value.Phase switch
        {
            SilvortexAccountPhase.Disabled => "Offline workspace",
            SilvortexAccountPhase.SignedOut => "No account session",
            SilvortexAccountPhase.Working => "Securing session...",
            SilvortexAccountPhase.SignedIn => value.DisplayName
                ?? value.Email
                ?? value.Subject
                ?? "Authenticated account",
            SilvortexAccountPhase.Error => "Account needs attention",
            _ => "Account unavailable",
        };
        statusText.Text = value.IsSignedIn
            ? value.Email ?? value.Subject ?? value.Message
            : value.Message;
        statusText.Foreground = value.Phase == SilvortexAccountPhase.Error
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Muted;
        actionButton.Content = value.Phase switch
        {
            SilvortexAccountPhase.Disabled => "Not configured",
            SilvortexAccountPhase.Working => "Working...",
            SilvortexAccountPhase.SignedIn => "Sign out",
            SilvortexAccountPhase.Error => "Retry",
            _ => "Sign in",
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
                ? "Sign out of Team Silvortex in Leserpent"
                : "Sign in to Team Silvortex");
        AutomationProperties.SetName(
            statusText,
            $"Team Silvortex account status: {identityText.Text}. {statusText.Text}");
    }
}
