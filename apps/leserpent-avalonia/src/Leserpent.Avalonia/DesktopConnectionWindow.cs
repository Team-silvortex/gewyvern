using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Platform.Storage;

internal sealed record DesktopConnectionRequest(
    string Endpoint,
    string CertificateAuthorityPath,
    string? BootstrapTrustRoot,
    string? BootstrapTrustHandle,
    string? Token,
    bool Remember);

internal sealed class DesktopConnectionWindow : Window
{
    private readonly DesktopLocalization localization;
    private readonly Func<DesktopConnectionRequest, string?> connect;
    private readonly Func<DesktopConnectionRequest, CancellationToken, Task<string?>>
        testConnection;
    private readonly DesktopConnectionProfile? savedProfile;
    private readonly Action? closeWindow;
    private readonly Func<string?>? forgetSavedConnection;
    private readonly bool usesBootstrapTrust;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<Control> auditedControls = [];
    private readonly TextBox endpoint = new()
    {
        MaxLength = 2048,
        PlaceholderText = "https://control.example:9443",
    };
    private readonly TextBox certificate = new()
    {
        MaxLength = 4096,
        PlaceholderText = "/path/to/ca.pem",
    };
    private readonly TextBox token = new()
    {
        MaxLength = 4096,
        PasswordChar = '\u2022',
    };
    private readonly TextBlock account = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 12,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock status = new()
    {
        Foreground = LeserpentTheme.Destructive,
        FontSize = 13,
        TextWrapping = TextWrapping.Wrap,
        IsVisible = false,
    };
    private readonly CheckBox remember = new()
    {
        IsChecked = true,
        Foreground = LeserpentTheme.Body,
    };
    private readonly Button connectButton = new()
    {
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(18, 9),
    };
    private readonly Button testButton = new()
    {
        Padding = new Thickness(18, 9),
    };
    private readonly Button browseButton = new()
    {
        Padding = new Thickness(14, 8),
    };
    private readonly Button closeButton = new()
    {
        Padding = new Thickness(18, 9),
    };
    private readonly Button forgetButton = new()
    {
        Foreground = LeserpentTheme.Destructive,
        Padding = new Thickness(14, 8),
    };
    private readonly TextBlock headingText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 25,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock descriptionText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 13,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock endpointLabel = CreateLabel();
    private readonly TextBlock certificateLabel = CreateLabel();
    private readonly TextBlock tokenLabel = CreateLabel();
    private bool operationInFlight;
    private bool isClosed;
    private bool statusIsError;
    private string? localizedStatusKey;

    public DesktopConnectionWindow(
        DesktopConnectionProfile? profile,
        string? initialError,
        Func<DesktopConnectionRequest, string?> connect,
        Func<DesktopConnectionRequest, CancellationToken, Task<string?>> testConnection,
        DesktopLocalization localization,
        Action? closeWindow = null,
        Func<string?>? forgetSavedConnection = null)
    {
        this.localization = localization;
        this.connect = connect;
        this.testConnection = testConnection;
        savedProfile = profile;
        this.closeWindow = closeWindow;
        this.forgetSavedConnection = forgetSavedConnection;
        usesBootstrapTrust = profile?.BootstrapTrustHandle is not null;
        Width = 640;
        MinWidth = 520;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        endpoint.Text = profile?.Endpoint ?? string.Empty;
        certificate.Text = profile?.CertificateAuthorityPath ?? string.Empty;
        if (usesBootstrapTrust)
        {
            certificate.IsReadOnly = true;
        }
        endpoint.TextChanged += (_, _) => UpdateAccount();
        browseButton.IsEnabled = !usesBootstrapTrust;
        forgetButton.IsVisible = profile is not null && forgetSavedConnection is not null;
        browseButton.Click += async (_, _) => await ChooseCertificateAsync();
        closeButton.Click += (_, _) => (closeWindow ?? Close)();
        forgetButton.Click += async (_, _) => await ConfirmForgetAsync(forgetButton);
        testButton.Click += async (_, _) => await TestConnectionAsync();
        connectButton.Click += (_, _) => Submit();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close();
            }
        };
        Opened += (_, _) => endpoint.Focus();
        Closed += (_, _) =>
        {
            isClosed = true;
            localization.Changed -= OnLocalizationChanged;
            lifetime.Cancel();
            lifetime.Dispose();
        };

        AutomationProperties.SetAutomationId(endpoint, "desktop-connect-endpoint");
        AutomationProperties.SetAutomationId(certificate, "desktop-connect-ca-path");
        AutomationProperties.SetAutomationId(token, "desktop-connect-token");
        AutomationProperties.SetAutomationId(browseButton, "desktop-connect-ca-browse");
        AutomationProperties.SetAutomationId(remember, "desktop-connect-remember");
        AutomationProperties.SetAutomationId(connectButton, "desktop-connect-submit");
        AutomationProperties.SetAutomationId(testButton, "desktop-connect-test");
        AutomationProperties.SetAutomationId(closeButton, "desktop-connect-close");
        AutomationProperties.SetAutomationId(forgetButton, "desktop-connect-forget");
        AutomationProperties.SetAutomationId(status, "desktop-connect-status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);
        auditedControls.AddRange(
            [endpoint, certificate, token, browseButton, remember, testButton, connectButton, closeButton, status]);
        if (forgetButton.IsVisible)
        {
            auditedControls.Add(forgetButton);
        }

        var certificateRow = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
            ColumnSpacing = 10,
            Children = { certificate, browseButton },
        };
        Grid.SetColumn(browseButton, 1);
        var buttons = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            Spacing = 12,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Children = { closeButton, testButton, connectButton },
        };
        Content = new Border
        {
            Padding = new Thickness(34, 30),
            Child = new StackPanel
            {
                Spacing = 12,
                Children =
                {
                    new TextBlock
                    {
                        Text = "LESERPENT",
                        Foreground = LeserpentTheme.Accent,
                        FontSize = 13,
                        FontWeight = FontWeight.Bold,
                        LetterSpacing = 2,
                    },
                    headingText,
                    descriptionText,
                    endpointLabel,
                    endpoint,
                    certificateLabel,
                    certificateRow,
                    tokenLabel,
                    token,
                    account,
                    remember,
                    forgetButton,
                    status,
                    buttons,
                },
            },
        };
        localization.Changed += OnLocalizationChanged;
        ApplyLocalization();
        UpdateAccount();
        if (!string.IsNullOrWhiteSpace(initialError))
        {
            ShowError(initialError);
        }
    }

    public void VerifyAccessibility()
    {
        var ids = new HashSet<string>(StringComparer.Ordinal);
        foreach (var control in auditedControls)
        {
            var id = AutomationProperties.GetAutomationId(control);
            var name = AutomationProperties.GetName(control);
            if (string.IsNullOrWhiteSpace(id)
                || string.IsNullOrWhiteSpace(name)
                || !ids.Add(id))
            {
                throw new InvalidDataException(
                    "desktop connection control accessibility metadata is incomplete");
            }
        }
        var expectedControlCount = forgetSavedConnection is null || savedProfile is null ? 9 : 10;
        if (auditedControls.Count != expectedControlCount
            || token.PasswordChar == default
            || AutomationProperties.GetLiveSetting(status) != AutomationLiveSetting.Assertive)
        {
            throw new InvalidDataException("desktop connection accessibility contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("desktop connection window has no control root");
        }
        root.Measure(new Size(Width, 1200));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 1200)
        {
            throw new InvalidDataException(
                "desktop connection controls exceeded their layout envelope");
        }
    }

    public void ProbeLocalizedPresentation(
        string expectedTitle,
        string expectedHeading,
        string expectedConnect)
    {
        if (Title != expectedTitle
            || headingText.Text != expectedHeading
            || connectButton.Content as string != expectedConnect
            || AutomationProperties.GetName(connectButton) != expectedConnect
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "desktop connection localized presentation drifted");
        }
    }

    public void ProbeSecureTokenSubmission(string fixtureToken)
    {
        token.Text = fixtureToken;
        Submit();
        if (!string.IsNullOrEmpty(token.Text))
        {
            throw new InvalidDataException("desktop token input was retained after submission");
        }
    }

    public async Task ProbeConnectionTestAsync()
    {
        await TestConnectionAsync();
        if (!status.IsVisible
            || status.Text != Text("status.ready")
            || operationInFlight)
        {
            throw new InvalidDataException("desktop connection test contract drifted");
        }
    }

    private async Task ChooseCertificateAsync()
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = Text("picker.title"),
            AllowMultiple = false,
            FileTypeFilter =
            [
                new FilePickerFileType(Text("picker.pem"))
                {
                    Patterns = ["*.pem", "*.crt", "*.cer"],
                },
            ],
        });
        var selected = files.FirstOrDefault()?.TryGetLocalPath();
        if (!string.IsNullOrWhiteSpace(selected))
        {
            certificate.Text = selected;
        }
    }

    private async Task ConfirmForgetAsync(Button forgetControl)
    {
        if (savedProfile is null || forgetSavedConnection is null)
        {
            return;
        }
        forgetControl.IsEnabled = false;
        var confirmation = new DesktopForgetConnectionWindow(
            savedProfile.Endpoint,
            forgetSavedConnection,
            localization);
        var forgotten = await confirmation.ShowDialog<bool>(this);
        forgetControl.IsEnabled = true;
        if (!forgotten)
        {
            return;
        }

        endpoint.Text = string.Empty;
        certificate.Text = string.Empty;
        token.Text = string.Empty;
        remember.IsChecked = true;
        forgetControl.IsVisible = false;
        ShowLocalizedStatus("status.removed", LeserpentTheme.Accent);
        endpoint.Focus();
    }

    private void Submit()
    {
        if (operationInFlight)
        {
            return;
        }
        operationInFlight = true;
        connectButton.IsEnabled = false;
        testButton.IsEnabled = false;
        status.IsVisible = false;
        var error = connect(Request(clearToken: true));
        if (error is not null)
        {
            ShowError(error);
            connectButton.IsEnabled = true;
            testButton.IsEnabled = true;
            operationInFlight = false;
        }
    }

    private async Task TestConnectionAsync()
    {
        if (operationInFlight || isClosed)
        {
            return;
        }
        operationInFlight = true;
        connectButton.IsEnabled = false;
        testButton.IsEnabled = false;
        ShowLocalizedStatus("status.testing", LeserpentTheme.Muted);
        try
        {
            var error = await testConnection(
                Request(clearToken: false),
                lifetime.Token);
            if (isClosed)
            {
                return;
            }
            if (error is not null)
            {
                ShowError(error);
                return;
            }
            ShowLocalizedStatus("status.ready", LeserpentTheme.Accent);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception)
        {
            if (!isClosed)
            {
                ShowLocalizedError("status.test_failed");
            }
        }
        finally
        {
            operationInFlight = false;
            if (!isClosed)
            {
                connectButton.IsEnabled = true;
                testButton.IsEnabled = true;
            }
        }
    }

    private DesktopConnectionRequest Request(bool clearToken)
    {
        var submittedToken = string.IsNullOrWhiteSpace(token.Text)
            ? null
            : token.Text;
        if (clearToken)
        {
            token.Text = string.Empty;
        }
        return new DesktopConnectionRequest(
            endpoint.Text?.Trim() ?? string.Empty,
            certificate.Text?.Trim() ?? string.Empty,
            savedProfile?.BootstrapTrustRoot,
            savedProfile?.BootstrapTrustHandle,
            submittedToken,
            remember.IsChecked == true);
    }

    private void UpdateAccount()
    {
        try
        {
            var value = RemoteClientOptions.ParseEndpoint(endpoint.Text?.Trim() ?? string.Empty);
            account.Text = Format("account", RemoteTokenResolver.Account(value));
        }
        catch (ArgumentException)
        {
            account.Text = Text("account.pending");
        }
    }

    private void ShowError(string value)
    {
        localizedStatusKey = null;
        statusIsError = true;
        status.Text = Safe(value);
        status.Foreground = LeserpentTheme.Destructive;
        status.IsVisible = true;
        RefreshStatusAutomationName();
    }

    private void ShowLocalizedError(string key)
    {
        localizedStatusKey = key;
        statusIsError = true;
        status.Text = Text(key);
        status.Foreground = LeserpentTheme.Destructive;
        status.IsVisible = true;
        RefreshStatusAutomationName();
    }

    private void ShowLocalizedStatus(string key, IBrush foreground)
    {
        localizedStatusKey = key;
        statusIsError = false;
        status.Text = Text(key);
        status.Foreground = foreground;
        status.IsVisible = true;
        RefreshStatusAutomationName();
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs) =>
        ApplyLocalization();

    private void ApplyLocalization()
    {
        Title = Text("title");
        FlowDirection = localization.FlowDirection;
        token.PlaceholderText = Text("token.placeholder");
        remember.Content = Text("remember");
        connectButton.Content = Text("connect");
        testButton.Content = Text("test");
        browseButton.Content = Text("choose_ca");
        closeButton.Content = Text(closeWindow is null ? "cancel" : "quit");
        forgetButton.Content = Text("forget_saved");
        headingText.Text = Text("heading");
        descriptionText.Text = Text(usesBootstrapTrust ? "body.bootstrap" : "body.standard");
        endpointLabel.Text = Text("endpoint.label");
        certificateLabel.Text = Text("ca.label");
        tokenLabel.Text = Text("token.label");
        certificate.PlaceholderText = usesBootstrapTrust
            ? Format("managed_ca", savedProfile!.BootstrapTrustHandle!)
            : "/path/to/ca.pem";

        AutomationProperties.SetName(endpoint, Text("endpoint.label"));
        AutomationProperties.SetName(certificate, Text("ca.label"));
        AutomationProperties.SetName(token, Text("token.label"));
        AutomationProperties.SetName(browseButton, Text("choose_ca"));
        AutomationProperties.SetName(remember, Text("remember"));
        AutomationProperties.SetName(connectButton, Text("connect"));
        AutomationProperties.SetName(testButton, Text("test"));
        AutomationProperties.SetHelpText(testButton, Text("test.help"));
        AutomationProperties.SetName(
            closeButton,
            Text(closeWindow is null ? "cancel" : "quit"));
        AutomationProperties.SetName(forgetButton, Text("forget_saved"));

        if (localizedStatusKey is not null)
        {
            status.Text = Text(localizedStatusKey);
        }
        RefreshStatusAutomationName();
        UpdateAccount();
    }

    private void RefreshStatusAutomationName()
    {
        var name = !status.IsVisible || string.IsNullOrWhiteSpace(status.Text)
            ? Text("status.name")
            : statusIsError
                ? Format("status.failed", Safe(status.Text))
                : status.Text;
        AutomationProperties.SetName(status, name);
    }

    private string Text(string key) =>
        DesktopConnectionCatalogs.Resolve(localization, key);

    private string Format(string key, params object[] values) =>
        DesktopConnectionCatalogs.Format(localization, key, values);

    private static TextBlock CreateLabel() => new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
    };

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(512)
        .ToArray());
}
