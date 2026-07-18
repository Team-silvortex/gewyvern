using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Platform.Storage;

internal sealed record DesktopConnectionRequest(
    string Endpoint,
    string CertificateAuthorityPath,
    string? Token,
    bool Remember);

internal sealed class DesktopConnectionWindow : Window
{
    private readonly Func<DesktopConnectionRequest, string?> connect;
    private readonly Func<DesktopConnectionRequest, CancellationToken, Task<string?>>
        testConnection;
    private readonly DesktopConnectionProfile? savedProfile;
    private readonly Func<string?>? forgetSavedConnection;
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
        PlaceholderText = "Leave blank to use the existing platform credential",
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
        Content = "Remember this non-secret connection profile",
        IsChecked = true,
        Foreground = LeserpentTheme.Body,
    };
    private readonly Button connectButton = new()
    {
        Content = "Connect",
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(18, 9),
    };
    private readonly Button testButton = new()
    {
        Content = "Test connection",
        Padding = new Thickness(18, 9),
    };
    private bool operationInFlight;
    private bool isClosed;

    public DesktopConnectionWindow(
        DesktopConnectionProfile? profile,
        string? initialError,
        Func<DesktopConnectionRequest, string?> connect,
        Func<DesktopConnectionRequest, CancellationToken, Task<string?>> testConnection,
        Action? closeWindow = null,
        Func<string?>? forgetSavedConnection = null)
    {
        this.connect = connect;
        this.testConnection = testConnection;
        savedProfile = profile;
        this.forgetSavedConnection = forgetSavedConnection;
        Title = "Leserpent / Connect";
        Width = 640;
        MinWidth = 520;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        endpoint.Text = profile?.Endpoint ?? string.Empty;
        certificate.Text = profile?.CertificateAuthorityPath ?? string.Empty;
        endpoint.TextChanged += (_, _) => UpdateAccount();

        var browse = new Button
        {
            Content = "Choose CA...",
            Padding = new Thickness(14, 8),
        };
        var close = new Button
        {
            Content = closeWindow is null ? "Cancel" : "Quit",
            Padding = new Thickness(18, 9),
        };
        var forget = new Button
        {
            Content = "Forget saved connection...",
            Foreground = LeserpentTheme.Destructive,
            IsVisible = profile is not null && forgetSavedConnection is not null,
            Padding = new Thickness(14, 8),
        };
        browse.Click += async (_, _) => await ChooseCertificateAsync();
        close.Click += (_, _) => (closeWindow ?? Close)();
        forget.Click += async (_, _) => await ConfirmForgetAsync(forget);
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
            lifetime.Cancel();
            lifetime.Dispose();
        };

        AutomationProperties.SetAutomationId(endpoint, "desktop-connect-endpoint");
        AutomationProperties.SetName(endpoint, "Remote HTTPS authority");
        AutomationProperties.SetAutomationId(certificate, "desktop-connect-ca-path");
        AutomationProperties.SetName(certificate, "Remote CA certificate path");
        AutomationProperties.SetAutomationId(token, "desktop-connect-token");
        AutomationProperties.SetName(token, "Endpoint-scoped remote token");
        AutomationProperties.SetAutomationId(browse, "desktop-connect-ca-browse");
        AutomationProperties.SetName(browse, "Choose remote CA certificate");
        AutomationProperties.SetAutomationId(remember, "desktop-connect-remember");
        AutomationProperties.SetName(remember, "Remember non-secret connection profile");
        AutomationProperties.SetAutomationId(connectButton, "desktop-connect-submit");
        AutomationProperties.SetName(connectButton, "Connect to remote authority");
        AutomationProperties.SetAutomationId(testButton, "desktop-connect-test");
        AutomationProperties.SetName(testButton, "Test remote authority connection");
        AutomationProperties.SetHelpText(
            testButton,
            "Checks TLS, authentication, protocol version, and authority readiness without saving the connection.");
        AutomationProperties.SetAutomationId(close, "desktop-connect-close");
        AutomationProperties.SetName(
            close,
            closeWindow is null ? "Cancel connection settings" : "Quit Leserpent");
        AutomationProperties.SetAutomationId(forget, "desktop-connect-forget");
        AutomationProperties.SetName(forget, "Forget saved connection and credential");
        AutomationProperties.SetAutomationId(status, "desktop-connect-status");
        AutomationProperties.SetName(status, "Connection setup status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);
        auditedControls.AddRange(
            [endpoint, certificate, token, browse, remember, testButton, connectButton, close, status]);
        if (forget.IsVisible)
        {
            auditedControls.Add(forget);
        }

        var certificateRow = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
            ColumnSpacing = 10,
            Children = { certificate, browse },
        };
        Grid.SetColumn(browse, 1);
        var buttons = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            Spacing = 12,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Children = { close, testButton, connectButton },
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
                    new TextBlock
                    {
                        Text = "Connect the desktop console",
                        Foreground = LeserpentTheme.Primary,
                        FontSize = 25,
                        FontWeight = FontWeight.Bold,
                    },
                    new TextBlock
                    {
                        Text = "Enter a token once to store it in macOS Keychain or Linux Secret Service. Remembered CA certificates are copied into private application storage; the profile stores only the HTTPS origin and managed CA path.",
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 13,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    Label("HTTPS authority"),
                    endpoint,
                    Label("CA certificate"),
                    certificateRow,
                    Label("Endpoint-scoped token (optional)"),
                    token,
                    account,
                    remember,
                    forget,
                    status,
                    buttons,
                },
            },
        };
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
            || status.Text != "Connection verified. The remote authority is ready."
            || operationInFlight)
        {
            throw new InvalidDataException("desktop connection test contract drifted");
        }
    }

    private async Task ChooseCertificateAsync()
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Choose the trusted CA certificate",
            AllowMultiple = false,
            FileTypeFilter =
            [
                new FilePickerFileType("PEM certificate")
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
            forgetSavedConnection);
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
        status.Text = "Saved profile and endpoint credential removed.";
        status.Foreground = LeserpentTheme.Accent;
        status.IsVisible = true;
        AutomationProperties.SetName(status, "Saved connection removed successfully");
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
        status.Text = "Testing TLS, authentication, and authority readiness...";
        status.Foreground = LeserpentTheme.Muted;
        status.IsVisible = true;
        AutomationProperties.SetName(status, "Testing remote connection");
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
            status.Text = "Connection verified. The remote authority is ready.";
            status.Foreground = LeserpentTheme.Accent;
            AutomationProperties.SetName(status, "Remote connection verified successfully");
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception)
        {
            if (!isClosed)
            {
                ShowError("Connection test failed safely.");
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
            submittedToken,
            remember.IsChecked == true);
    }

    private void UpdateAccount()
    {
        try
        {
            var value = RemoteClientOptions.ParseEndpoint(endpoint.Text?.Trim() ?? string.Empty);
            account.Text = $"Credential account: {RemoteTokenResolver.Account(value)}";
        }
        catch (ArgumentException)
        {
            account.Text = "Credential account appears after a valid HTTPS origin is entered.";
        }
    }

    private void ShowError(string value)
    {
        status.Text = Safe(value);
        status.Foreground = LeserpentTheme.Destructive;
        status.IsVisible = true;
        AutomationProperties.SetName(status, $"Connection setup failed: {Safe(value)}");
    }

    private static TextBlock Label(string value) => new()
    {
        Text = value,
        Foreground = LeserpentTheme.Body,
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
    };

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(512)
        .ToArray());
}
