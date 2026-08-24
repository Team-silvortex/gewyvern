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
    private readonly Button installLanguagePackButton = new()
    {
        Padding = new Thickness(16, 8),
    };
    private readonly Button removeLanguagePackButton = new()
    {
        Padding = new Thickness(16, 8),
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

    public DesktopLanguageWindow(DesktopLocalization localization, Action applied)
    {
        this.localization = localization;
        this.applied = applied;
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
            "Language preference status");
        ConfigureControl(
            languagePackText,
            "desktop-language-pack-status",
            localization.Text(DesktopTextKey.LanguagePacks));
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
        installLanguagePackButton.IsEnabled = localization.SupportsLanguagePackInstallation;
        languageBox.SelectionChanged += (_, _) => UpdateCoverage();
        cancelButton.Click += (_, _) => Close();
        applyButton.Click += (_, _) => ApplySelection();
        installLanguagePackButton.Click += async (_, _) => await InstallLanguagePackAsync();
        removeLanguagePackButton.Click += (_, _) => RemoveSelectedLanguagePack();

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
                    new StackPanel
                    {
                        Orientation = Orientation.Horizontal,
                        Spacing = 10,
                        Children =
                        {
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
            || auditedControls.Count != 8
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
        if (!localization.SupportsLanguagePackInstallation)
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
            await using var stream = await files[0].OpenReadAsync();
            _ = await InstallLanguagePackAsync(stream);
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

    private async Task<DesktopInstalledLanguagePack> InstallLanguagePackAsync(Stream stream)
    {
        var installed = await localization.InstallLanguagePackAsync(stream);
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
        if (languageBox.SelectedItem is not DesktopLanguageChoice choice
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
            removeLanguagePackButton.IsEnabled = false;
            return;
        }
        var version = localization.InstalledLanguagePackVersion(locale.Locale);
        languagePackText.Text = version is null
            ? localization.Text(DesktopTextKey.LanguagePackNotInstalled)
            : localization.Format(DesktopTextKey.LanguagePackInstalled, version);
        removeLanguagePackButton.IsEnabled = version is not null;
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
            statusText.Text = "Select an official language first.";
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
            statusText.Text = $"Language preference was not saved: {error.Message}";
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
