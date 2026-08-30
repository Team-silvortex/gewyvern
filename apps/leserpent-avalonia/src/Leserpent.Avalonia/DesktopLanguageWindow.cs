using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Platform.Storage;

internal sealed record DesktopLanguageChoice(
    string Preference,
    DesktopLocaleDefinition Locale,
    string DisplayName)
{
    public override string ToString() => DisplayName;
}

internal sealed class DesktopLanguageWindow : Window
{
    private readonly DesktopLocalization localization;
    private readonly Action applied;
    private readonly IReadOnlyList<DesktopLanguagePackSource> languagePackSources;
    private readonly Func<
        DesktopLanguagePackSource,
        string,
        CancellationToken,
        Task<DesktopLanguagePackDownload>>? downloadLanguagePack;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<Control> auditedControls = [];
    private readonly ComboBox languageBox = new()
    {
        MinWidth = 310,
        HorizontalAlignment = HorizontalAlignment.Stretch,
    };
    private readonly TextBlock coverageText = new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 13,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock statusText = new()
    {
        Foreground = LeserpentTheme.Destructive,
        FontSize = 12,
        TextWrapping = TextWrapping.Wrap,
        IsVisible = false,
    };
    private readonly TextBlock languagePackText = new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 13,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly ComboBox languagePackSourceBox = new()
    {
        MinWidth = 310,
        HorizontalAlignment = HorizontalAlignment.Stretch,
    };
    private readonly Button downloadLanguagePackButton = new()
    {
        Padding = new Thickness(16, 8),
        Margin = new Thickness(0, 0, 10, 8),
    };
    private readonly Button installLanguagePackButton = new()
    {
        Padding = new Thickness(16, 8),
        Margin = new Thickness(0, 0, 10, 8),
    };
    private readonly Button removeLanguagePackButton = new()
    {
        Padding = new Thickness(16, 8),
        Margin = new Thickness(0, 0, 0, 8),
    };
    private readonly Button cancelButton = new()
    {
        Padding = new Thickness(18, 8),
    };
    private readonly Button applyButton = new()
    {
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(22, 8),
    };
    private readonly IReadOnlyList<DesktopLanguageChoice> choices;
    private bool languagePackOperationInProgress;

    public DesktopLanguageWindow(DesktopLocalization localization, Action applied)
        : this(localization, applied, [], null)
    {
    }

    public DesktopLanguageWindow(
        DesktopLocalization localization,
        Action applied,
        IReadOnlyList<DesktopLanguagePackSource> languagePackSources,
        Func<
            DesktopLanguagePackSource,
            string,
            CancellationToken,
            Task<DesktopLanguagePackDownload>>? downloadLanguagePack)
    {
        this.localization = localization;
        this.applied = applied;
        this.languagePackSources = languagePackSources.ToArray();
        this.downloadLanguagePack = downloadLanguagePack;
        if ((this.languagePackSources.Count > 0 && downloadLanguagePack is null)
            || this.languagePackSources.Select(source => source.SourceId)
                .Distinct(StringComparer.Ordinal).Count() != this.languagePackSources.Count
            || this.languagePackSources.Select(source => source.Endpoint)
                .Distinct().Count() != this.languagePackSources.Count)
        {
            throw new InvalidDataException(
                "desktop language-pack sources are inconsistent");
        }
        choices = BuildChoices(localization);
        Title = localization.Text(DesktopTextKey.LanguageSettingsTitle);
        Width = 610;
        MinWidth = 480;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        FlowDirection = localization.FlowDirection;

        languageBox.ItemsSource = choices;
        languageBox.SelectedItem = choices.Single(choice =>
            choice.Preference == localization.Preference);
        languagePackSourceBox.ItemsSource = this.languagePackSources;
        languagePackSourceBox.SelectedItem = this.languagePackSources.FirstOrDefault();
        ConfigureControl(
            languageBox,
            "desktop-language-choice",
            localization.Text(DesktopTextKey.LanguagePreference));
        ConfigureControl(
            coverageText,
            "desktop-language-coverage",
            localization.Text(DesktopTextKey.DesktopCoverage));
        ConfigureControl(
            statusText,
            "desktop-language-status",
            localization.Text(DesktopTextKey.LanguagePreference));
        ConfigureControl(
            languagePackText,
            "desktop-language-pack-status",
            localization.Text(DesktopTextKey.LanguagePacks));
        ConfigureControl(
            languagePackSourceBox,
            "desktop-language-pack-source",
            localization.Text(DesktopTextKey.LanguagePackSource));
        ConfigureControl(
            downloadLanguagePackButton,
            "desktop-language-pack-download",
            localization.Text(DesktopTextKey.DownloadLanguagePack));
        ConfigureControl(
            installLanguagePackButton,
            "desktop-language-pack-install",
            localization.Text(DesktopTextKey.InstallLanguagePack));
        ConfigureControl(
            removeLanguagePackButton,
            "desktop-language-pack-remove",
            localization.Text(DesktopTextKey.RemoveLanguagePack));
        ConfigureControl(
            cancelButton,
            "desktop-language-cancel",
            localization.Text(DesktopTextKey.Cancel));
        ConfigureControl(
            applyButton,
            "desktop-language-apply",
            localization.Text(DesktopTextKey.Apply));
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Polite);
        cancelButton.Content = localization.Text(DesktopTextKey.Cancel);
        applyButton.Content = localization.Text(DesktopTextKey.Apply);
        installLanguagePackButton.Content = localization.Text(
            DesktopTextKey.InstallLanguagePack);
        removeLanguagePackButton.Content = localization.Text(
            DesktopTextKey.RemoveLanguagePack);
        downloadLanguagePackButton.Content = localization.Text(
            DesktopTextKey.DownloadLanguagePack);
        languageBox.SelectionChanged += (_, _) => UpdateCoverage();
        languagePackSourceBox.SelectionChanged += (_, _) => UpdateActionAvailability();
        cancelButton.Click += (_, _) => Close();
        applyButton.Click += (_, _) => ApplySelection();
        installLanguagePackButton.Click += async (_, _) => await InstallLanguagePackAsync();
        downloadLanguagePackButton.Click += async (_, _) =>
            await DownloadSelectedLanguagePackAsync();
        removeLanguagePackButton.Click += (_, _) => RemoveSelectedLanguagePack();
        Closed += (_, _) => lifetime.Cancel();

        var header = new StackPanel
        {
            Spacing = 6,
            Children =
            {
                new TextBlock
                {
                    Text = localization.Text(DesktopTextKey.LanguageSettingsKicker),
                    Foreground = LeserpentTheme.Accent,
                    FontSize = 12,
                    FontWeight = FontWeight.Bold,
                    LetterSpacing = 1.8,
                },
                new TextBlock
                {
                    Text = localization.Text(DesktopTextKey.LanguageSettingsHeading),
                    Foreground = LeserpentTheme.Primary,
                    FontSize = 26,
                    FontWeight = FontWeight.Bold,
                    TextWrapping = TextWrapping.Wrap,
                },
                new TextBlock
                {
                    Text = localization.Text(DesktopTextKey.LanguageSettingsBody),
                    Foreground = LeserpentTheme.Muted,
                    FontSize = 13,
                    TextWrapping = TextWrapping.Wrap,
                },
            },
        };
        var picker = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(12),
            Padding = new Thickness(20, 18),
            Child = new StackPanel
            {
                Spacing = 9,
                Children =
                {
                    new TextBlock
                    {
                        Text = localization.Text(DesktopTextKey.LanguagePreference),
                        Foreground = LeserpentTheme.Primary,
                        FontWeight = FontWeight.SemiBold,
                    },
                    languageBox,
                    new TextBlock
                    {
                        Text = localization.Text(DesktopTextKey.DesktopCoverage),
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 12,
                        FontWeight = FontWeight.SemiBold,
                        Margin = new Thickness(0, 7, 0, 0),
                    },
                    coverageText,
                    new TextBlock
                    {
                        Text = localization.Text(DesktopTextKey.AppliesImmediately),
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 12,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new Border
                    {
                        Height = 1,
                        Margin = new Thickness(0, 8, 0, 2),
                        Background = LeserpentTheme.PanelBorder,
                    },
                    new TextBlock
                    {
                        Text = localization.Text(DesktopTextKey.LanguagePacks),
                        Foreground = LeserpentTheme.Primary,
                        FontWeight = FontWeight.SemiBold,
                    },
                    languagePackText,
                    new TextBlock
                    {
                        Text = localization.Text(DesktopTextKey.LanguagePackSource),
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 12,
                        FontWeight = FontWeight.SemiBold,
                    },
                    languagePackSourceBox,
                    new TextBlock
                    {
                        Text = localization.Text(DesktopTextKey.LanguagePackSourceHint),
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 12,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new WrapPanel
                    {
                        Orientation = Orientation.Horizontal,
                        Children =
                        {
                            downloadLanguagePackButton,
                            installLanguagePackButton,
                            removeLanguagePackButton,
                        },
                    },
                    statusText,
                },
            },
        };
        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 10,
            HorizontalAlignment = HorizontalAlignment.Right,
            Children = { cancelButton, applyButton },
        };
        Content = new StackPanel
        {
            Spacing = 18,
            Margin = new Thickness(30, 26),
            Children = { header, picker, buttons },
        };
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close();
            }
        };
        UpdateCoverage();
    }

    public void VerifyAccessibility()
    {
        var ids = new HashSet<string>(StringComparer.Ordinal);
        if (choices.Count != 31
            || auditedControls.Count != 10
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))
                || !ids.Add(AutomationProperties.GetAutomationId(control)!))
            || LeserpentTheme.MinimumTextContrastRatio < 4.5)
        {
            throw new InvalidDataException("desktop language controls drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("desktop language window has no control root");
        }
        root.Measure(new Size(Width, 900));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 900)
        {
            throw new InvalidDataException("desktop language controls exceeded their layout envelope");
        }
    }

    public void ProbeSelectionContract()
    {
        languageBox.SelectedItem = choices.Single(choice => choice.Preference == "zh-CN");
        if (languageBox.SelectedItem is not DesktopLanguageChoice { Preference: "zh-CN" }
            || !coverageText.Text!.Contains("简体中文", StringComparison.Ordinal))
        {
            throw new InvalidDataException("desktop language selector did not expose its choice");
        }
        applyButton.RaiseEvent(new Avalonia.Interactivity.RoutedEventArgs(Button.ClickEvent));
        if (localization.Preference != "zh-CN"
            || localization.Text(DesktopTextKey.ControlTopology) != "控制拓扑")
        {
            throw new InvalidDataException("desktop language selector did not apply immediately");
        }
    }

    public void ProbeLocalizedFailurePresentation()
    {
        var selected = languageBox.SelectedItem;
        var saveFailure = localization.Format(
            DesktopTextKey.LanguagePreferenceSaveFailed,
            "fixture");
        languageBox.SelectedItem = null;
        ApplySelection();
        if (statusText.Text != localization.Text(DesktopTextKey.LanguageSelectionRequired)
            || !statusText.IsVisible
            || AutomationProperties.GetName(statusText)
                != localization.Text(DesktopTextKey.LanguagePreference)
            || !saveFailure.Contains("fixture", StringComparison.Ordinal)
            || saveFailure.Any(char.IsControl))
        {
            throw new InvalidDataException(
                "desktop language failure presentation was not localized");
        }
        languageBox.SelectedItem = selected;
        statusText.IsVisible = false;
    }

    public void ProbeLanguagePackContract(ReadOnlySpan<byte> payload)
    {
        var installed = InstallLanguagePack(payload);
        if (languageBox.SelectedItem is not DesktopLanguageChoice choice
            || choice.Locale.Locale != installed.Manifest.Locale
            || !localization.IsLanguagePackInstalled(installed.Manifest.Locale)
            || languagePackText.Text
                != localization.Format(
                    DesktopTextKey.LanguagePackInstalled,
                    installed.Manifest.Version)
            || !removeLanguagePackButton.IsEnabled)
        {
            throw new InvalidDataException(
                "desktop language-pack controls did not expose the installed pack");
        }
        removeLanguagePackButton.RaiseEvent(
            new Avalonia.Interactivity.RoutedEventArgs(Button.ClickEvent));
        if (localization.IsLanguagePackInstalled(installed.Manifest.Locale)
            || removeLanguagePackButton.IsEnabled)
        {
            throw new InvalidDataException(
                "desktop language-pack controls did not remove the selected pack");
        }
    }

    public async Task ProbeLanguagePackDownloadContractAsync()
    {
        if (languagePackSources.Count != 1 || downloadLanguagePack is null)
        {
            throw new InvalidDataException(
                "desktop language-pack download probe has no source");
        }
        SelectLocale("pt-BR");
        await DownloadSelectedLanguagePackAsync();
        var source = languagePackSources[0];
        if (!localization.IsLanguagePackInstalled("pt-BR")
            || !Equals(languagePackSourceBox.SelectedItem, source)
            || statusText.Text is null
            || !statusText.Text.Contains(source.DisplayName, StringComparison.Ordinal)
            || downloadLanguagePackButton.IsEnabled == false)
        {
            throw new InvalidDataException(
                "desktop language-pack catalog download did not reach native controls");
        }
    }

    public async Task ProbeLanguagePackCancellationContractAsync(string locale)
    {
        if (languagePackSources.Count != 1 || downloadLanguagePack is null)
        {
            throw new InvalidDataException(
                "desktop language-pack cancellation probe has no source");
        }
        SelectLocale(locale);
        var pending = DownloadSelectedLanguagePackAsync();
        lifetime.Cancel();
        await pending;
        if (localization.IsLanguagePackInstalled(locale)
            || languagePackOperationInProgress)
        {
            throw new InvalidDataException(
                "desktop language-pack cancellation committed an installation");
        }
    }

    private static IReadOnlyList<DesktopLanguageChoice> BuildChoices(
        DesktopLocalization localization)
    {
        var result = new List<DesktopLanguageChoice>
        {
            new(
                DesktopLocalization.SystemPreference,
                localization.Active,
                $"{localization.Text(DesktopTextKey.FollowSystem)} · {localization.Active.NativeName}"),
        };
        result.AddRange(DesktopLocalization.OfficialLocales.Select(locale => new DesktopLanguageChoice(
            locale.Locale,
            locale,
            $"{locale.NativeName} · {locale.Name}")));
        return result;
    }

    private void UpdateCoverage()
    {
        if (languageBox.SelectedItem is not DesktopLanguageChoice choice)
        {
            coverageText.Text = string.Empty;
            languagePackText.Text = string.Empty;
            removeLanguagePackButton.IsEnabled = false;
            UpdateActionAvailability();
            return;
        }
        var coverage = choice.Locale.Coverage switch
        {
            DesktopLocaleCoverage.Complete => DesktopTextKey.CoverageComplete,
            DesktopLocaleCoverage.Core => DesktopTextKey.CoverageCore,
            _ => DesktopTextKey.CoverageFallback,
        };
        coverageText.Text = $"{choice.Locale.NativeName} ({choice.Locale.Locale}) · {localization.Text(coverage)}";
        UpdateLanguagePackState(choice.Locale);
    }

    private async Task InstallLanguagePackAsync()
    {
        if (!BeginLanguagePackOperation())
        {
            return;
        }
        try
        {
            var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
            {
                Title = localization.Text(DesktopTextKey.InstallLanguagePack),
                AllowMultiple = false,
                FileTypeFilter =
                [
                    new FilePickerFileType(
                        localization.Text(DesktopTextKey.LanguagePacks))
                    {
                        Patterns = ["*.json"],
                        AppleUniformTypeIdentifiers = ["public.json"],
                        MimeTypes = ["application/json"],
                    },
                ],
            });
            if (files.Count != 1)
            {
                return;
            }
            lifetime.Token.ThrowIfCancellationRequested();
            await using var stream = await files[0].OpenReadAsync();
            _ = await InstallLanguagePackAsync(stream);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error) when (lifetime.IsCancellationRequested
            && StartupFailure.IsExpected(error))
        {
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            SetStatus(
                localization.Format(
                    DesktopTextKey.LanguagePackOperationFailed,
                    StartupFailure.Describe(error)),
                failed: true);
        }
        finally
        {
            EndLanguagePackOperation();
        }
    }

    private async Task DownloadSelectedLanguagePackAsync()
    {
        if (languageBox.SelectedItem is not DesktopLanguageChoice choice
            || choice.Locale.BuiltIn
            || languagePackSourceBox.SelectedItem is not DesktopLanguagePackSource source
            || downloadLanguagePack is null
            || !BeginLanguagePackOperation())
        {
            return;
        }
        try
        {
            var downloaded = await downloadLanguagePack(
                source,
                choice.Locale.Locale,
                lifetime.Token);
            lifetime.Token.ThrowIfCancellationRequested();
            if (downloaded.SourceId != source.SourceId
                || downloaded.Locale != choice.Locale.Locale)
            {
                throw new InvalidDataException(
                    "language-pack download did not match its selected source and locale");
            }
            var installed = localization.InstallCatalogLanguagePack(
                downloaded.Payload,
                downloaded.Sha256,
                downloaded.Locale,
                downloaded.Version,
                lifetime.Token);
            SelectLocale(installed.Manifest.Locale);
            SetStatus(
                localization.Format(
                    DesktopTextKey.LanguagePackDownloadSucceeded,
                    installed.Manifest.NativeName,
                    source.DisplayName),
                failed: false);
            applied();
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error) when (lifetime.IsCancellationRequested
            && (StartupFailure.IsExpected(error)
                || error is HttpRequestException or TaskCanceledException))
        {
        }
        catch (Exception error) when (StartupFailure.IsExpected(error)
            || error is HttpRequestException or TaskCanceledException)
        {
            SetStatus(
                localization.Format(
                    DesktopTextKey.LanguagePackOperationFailed,
                    StartupFailure.Describe(error)),
                failed: true);
        }
        finally
        {
            EndLanguagePackOperation();
        }
    }

    private async Task<DesktopInstalledLanguagePack> InstallLanguagePackAsync(Stream stream)
    {
        var installed = await localization.InstallLanguagePackAsync(
            stream,
            cancellationToken: lifetime.Token);
        lifetime.Token.ThrowIfCancellationRequested();
        SelectLocale(installed.Manifest.Locale);
        SetStatus(
            localization.Format(
                DesktopTextKey.LanguagePackInstallSucceeded,
                installed.Manifest.NativeName),
            failed: false);
        applied();
        return installed;
    }

    private DesktopInstalledLanguagePack InstallLanguagePack(ReadOnlySpan<byte> payload)
    {
        var installed = localization.InstallLanguagePack(payload);
        SelectLocale(installed.Manifest.Locale);
        SetStatus(
            localization.Format(
                DesktopTextKey.LanguagePackInstallSucceeded,
                installed.Manifest.NativeName),
            failed: false);
        applied();
        return installed;
    }

    private void RemoveSelectedLanguagePack()
    {
        if (languagePackOperationInProgress
            || languageBox.SelectedItem is not DesktopLanguageChoice choice
            || choice.Locale.BuiltIn
            || !localization.IsLanguagePackInstalled(choice.Locale.Locale))
        {
            return;
        }
        try
        {
            localization.RemoveLanguagePack(choice.Locale.Locale);
            SetStatus(
                localization.Format(
                    DesktopTextKey.LanguagePackRemoveSucceeded,
                    choice.Locale.NativeName),
                failed: false);
            UpdateLanguagePackState(choice.Locale);
            applied();
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            SetStatus(
                localization.Format(
                    DesktopTextKey.LanguagePackOperationFailed,
                    StartupFailure.Describe(error)),
                failed: true);
        }
    }

    private void SelectLocale(string locale)
    {
        languageBox.SelectedItem = choices.Single(choice =>
            choice.Preference.Equals(locale, StringComparison.OrdinalIgnoreCase));
        UpdateCoverage();
    }

    private void UpdateLanguagePackState(DesktopLocaleDefinition locale)
    {
        if (locale.BuiltIn)
        {
            languagePackText.Text = localization.Text(DesktopTextKey.BuiltInLanguagePack);
            UpdateActionAvailability();
            return;
        }
        var version = localization.InstalledLanguagePackVersion(locale.Locale);
        languagePackText.Text = version is null
            ? localization.Text(DesktopTextKey.LanguagePackNotInstalled)
            : localization.Format(DesktopTextKey.LanguagePackInstalled, version);
        UpdateActionAvailability();
    }

    private bool BeginLanguagePackOperation()
    {
        if (languagePackOperationInProgress
            || !localization.SupportsLanguagePackInstallation)
        {
            return false;
        }
        languagePackOperationInProgress = true;
        UpdateActionAvailability();
        return true;
    }

    private void EndLanguagePackOperation()
    {
        languagePackOperationInProgress = false;
        if (!lifetime.IsCancellationRequested)
        {
            UpdateCoverage();
        }
    }

    private void UpdateActionAvailability()
    {
        var selectedLocale = (languageBox.SelectedItem as DesktopLanguageChoice)?.Locale;
        var installed = selectedLocale is not null
            && !selectedLocale.BuiltIn
            && localization.IsLanguagePackInstalled(selectedLocale.Locale);
        languageBox.IsEnabled = !languagePackOperationInProgress;
        languagePackSourceBox.IsEnabled = !languagePackOperationInProgress
            && languagePackSources.Count > 0;
        installLanguagePackButton.IsEnabled = !languagePackOperationInProgress
            && localization.SupportsLanguagePackInstallation;
        downloadLanguagePackButton.IsEnabled = !languagePackOperationInProgress
            && localization.SupportsLanguagePackInstallation
            && downloadLanguagePack is not null
            && languagePackSourceBox.SelectedItem is DesktopLanguagePackSource
            && selectedLocale is { BuiltIn: false };
        removeLanguagePackButton.IsEnabled = !languagePackOperationInProgress && installed;
        applyButton.IsEnabled = !languagePackOperationInProgress;
    }

    private void SetStatus(string value, bool failed)
    {
        statusText.Text = value;
        statusText.Foreground = failed ? LeserpentTheme.Destructive : LeserpentTheme.Body;
        statusText.IsVisible = true;
    }

    private void ApplySelection()
    {
        if (languageBox.SelectedItem is not DesktopLanguageChoice choice)
        {
            statusText.Text = localization.Text(DesktopTextKey.LanguageSelectionRequired);
            statusText.IsVisible = true;
            return;
        }
        try
        {
            localization.SetPreference(choice.Preference);
            Close();
            applied();
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            statusText.Text = localization.Format(
                DesktopTextKey.LanguagePreferenceSaveFailed,
                StartupFailure.Describe(error));
            statusText.IsVisible = true;
        }
    }

    private void ConfigureControl(Control control, string automationId, string name)
    {
        AutomationProperties.SetAutomationId(control, automationId);
        AutomationProperties.SetName(control, name);
        auditedControls.Add(control);
    }
}
