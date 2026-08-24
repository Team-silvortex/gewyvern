using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;

internal sealed record DesktopTutorialStep(
    string Label,
    string Title,
    string Summary,
    IReadOnlyList<string> Points,
    string Model);

internal sealed class DesktopTutorialWindow : Window
{
    private readonly DesktopLocalization localization;
    private readonly List<Control> auditedControls = [];
    private readonly List<Button> stepButtons = [];
    private readonly TextBlock progressText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 12,
        FontWeight = FontWeight.SemiBold,
        VerticalAlignment = VerticalAlignment.Center,
    };
    private readonly TextBlock lessonLabel = new()
    {
        Foreground = LeserpentTheme.Accent,
        FontSize = 12,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 1.8,
    };
    private readonly TextBlock lessonTitle = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 28,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock lessonSummary = new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 15,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly StackPanel lessonPoints = new() { Spacing = 11 };
    private readonly TextBlock modelText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontFamily = new FontFamily("Menlo, Cascadia Mono, monospace"),
        FontSize = 13,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock tutorialKicker = new();
    private readonly TextBlock tutorialHeading = new();
    private readonly TextBlock tutorialBody = new();
    private readonly Button previousButton = new()
    {
        Content = string.Empty,
        Padding = new Thickness(16, 8),
    };
    private readonly Button nextButton = new()
    {
        Content = string.Empty,
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(20, 8),
    };
    private readonly Button closeButton = new()
    {
        Content = string.Empty,
        Padding = new Thickness(16, 8),
    };
    private int selectedStep;
    private DesktopTutorialStep[] currentSteps;
    private DesktopTutorialStep[] CurrentSteps => currentSteps;

    public DesktopTutorialWindow(DesktopLocalization? localization = null)
    {
        this.localization = localization ?? DesktopLocalization.ForVerification();
        currentSteps = DesktopTutorialCatalogs.Steps(this.localization);
        VerifyContentContract();
        Title = $"Leserpent / {this.localization.Text(DesktopTextKey.LearningCenter).TrimEnd('.')}";
        Width = 820;
        Height = 620;
        MinWidth = 520;
        MinHeight = 460;
        SizeToContent = SizeToContent.Manual;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        CanResize = true;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        FlowDirection = this.localization.FlowDirection;

        ConfigureNavigationControl(
            progressText,
            "desktop-tutorial-progress",
            TutorialText("a11y.progress"),
            TutorialText("help.progress"));
        AutomationProperties.SetLiveSetting(progressText, AutomationLiveSetting.Polite);
        ConfigureNavigationControl(
            previousButton,
            "desktop-tutorial-previous",
            TutorialText("a11y.previous"),
            TutorialText("help.previous"));
        ConfigureNavigationControl(
            nextButton,
            "desktop-tutorial-next",
            TutorialText("a11y.next"),
            TutorialText("help.next"));
        ConfigureNavigationControl(
            closeButton,
            "desktop-tutorial-close",
            TutorialText("a11y.close"),
            TutorialText("help.close"));
        previousButton.Click += (_, _) => ShowStep(selectedStep - 1);
        nextButton.Click += (_, _) =>
        {
            if (selectedStep == CurrentSteps.Length - 1)
            {
                Close();
                return;
            }
            ShowStep(selectedStep + 1);
        };
        closeButton.Click += (_, _) => Close();

        var stepGrid = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,*,*,*,*,*"),
            ColumnSpacing = 8,
        };
        for (var index = 0; index < CurrentSteps.Length; index++)
        {
            var capturedIndex = index;
            var button = new Button
            {
                Content = $"{index + 1:00}",
                HorizontalContentAlignment = HorizontalAlignment.Center,
                Padding = new Thickness(10, 7),
            };
            ConfigureNavigationControl(
                button,
                $"desktop-tutorial-step-{index + 1}",
                TutorialFormat(
                    "a11y.step.open",
                    index + 1,
                    CurrentSteps[index].Title),
                TutorialFormat(
                    "help.step.jump",
                    CurrentSteps[index].Label));
            ToolTip.SetTip(button, CurrentSteps[index].Title);
            button.Click += (_, _) => ShowStep(capturedIndex);
            Grid.SetColumn(button, index);
            stepGrid.Children.Add(button);
            stepButtons.Add(button);
        }

        var lesson = new StackPanel
        {
            Spacing = 16,
            Children =
            {
                lessonLabel,
                lessonTitle,
                lessonSummary,
                new Border
                {
                    Height = 1,
                    Background = LeserpentTheme.PanelBorder,
                    Margin = new Thickness(0, 2),
                },
                lessonPoints,
                new Border
                {
                    Background = Brush.Parse("#2A2113"),
                    BorderBrush = LeserpentTheme.PanelBorder,
                    BorderThickness = new Thickness(1),
                    CornerRadius = new CornerRadius(8),
                    Padding = new Thickness(14, 11),
                    Child = modelText,
                },
            },
        };
        var lessonCard = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(12),
            Padding = new Thickness(24, 22),
            Child = new ScrollViewer
            {
                Content = lesson,
                VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
            },
        };
        tutorialKicker.Text = this.localization.Text(DesktopTextKey.TutorialKicker);
        tutorialKicker.Foreground = LeserpentTheme.Accent;
        tutorialKicker.FontSize = 13;
        tutorialKicker.FontWeight = FontWeight.Bold;
        tutorialKicker.LetterSpacing = 2;
        tutorialHeading.Text = this.localization.Text(DesktopTextKey.TutorialHeading);
        tutorialHeading.Foreground = LeserpentTheme.Primary;
        tutorialHeading.FontSize = 25;
        tutorialHeading.FontWeight = FontWeight.Bold;
        tutorialBody.Text = this.localization.Text(DesktopTextKey.TutorialBody);
        tutorialBody.Foreground = LeserpentTheme.Muted;
        tutorialBody.FontSize = 13;
        tutorialBody.TextWrapping = TextWrapping.Wrap;
        var header = new StackPanel
        {
            Spacing = 5,
            Children =
            {
                tutorialKicker,
                tutorialHeading,
                tutorialBody,
            },
        };
        var footer = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("Auto,*,Auto,Auto"),
            ColumnSpacing = 10,
            Children = { previousButton, progressText, closeButton, nextButton },
        };
        Grid.SetColumn(progressText, 1);
        Grid.SetColumn(closeButton, 2);
        Grid.SetColumn(nextButton, 3);
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,*,Auto"),
            RowSpacing = 16,
            Margin = new Thickness(28, 24),
            Children = { header, stepGrid, lessonCard, footer },
        };
        Grid.SetRow(stepGrid, 1);
        Grid.SetRow(lessonCard, 2);
        Grid.SetRow(footer, 3);
        KeyDown += OnKeyDown;
        this.localization.Changed += OnLocalizationChanged;
        Closed += (_, _) => this.localization.Changed -= OnLocalizationChanged;
        ApplyLocalization();
        ShowStep(0);
    }

    public static void VerifyContentContract()
    {
        DesktopTutorialCatalogs.VerifyContract();
        foreach (var locale in DesktopLocalization.OfficialLocales.Where(
            locale => locale.BuiltIn))
        {
            var steps = DesktopTutorialCatalogs.Steps(
                DesktopLocalization.ForVerification(locale.Locale));
            var titles = new HashSet<string>(StringComparer.Ordinal);
            if (steps.Length != 6
                || steps.Any(step => !ValidText(step.Label, 40)
                    || !ValidText(step.Title, 80)
                    || !titles.Add(step.Title)
                    || !ValidText(step.Summary, 500)
                    || step.Points.Count is < 3 or > 5
                    || step.Points.Any(point => !ValidText(point, 240))
                    || !ValidText(step.Model, 160)))
            {
                throw new InvalidDataException("desktop tutorial content contract drifted");
            }
        }
        var completeText = string.Join(
            '\n',
            DesktopTutorialCatalogs.Steps(DesktopLocalization.ForVerification())
                .SelectMany(step => step.Points.Prepend(step.Summary)));
        foreach (var required in new[]
        {
            "leserpentd",
            "Gewyvern",
            "authenticated capability",
            "Leselang",
            "Automation IDs",
        })
        {
            if (!completeText.Contains(required, StringComparison.Ordinal))
            {
                throw new InvalidDataException(
                    $"desktop tutorial omitted required concept {required}");
            }
        }
    }

    public void VerifyAccessibility()
    {
        var ids = new HashSet<string>(StringComparer.Ordinal);
        if (auditedControls.Count != CurrentSteps.Length + 4
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetHelpText(control))
                || !ids.Add(AutomationProperties.GetAutomationId(control)!))
            || LeserpentTheme.MinimumTextContrastRatio < 4.5)
        {
            throw new InvalidDataException("desktop tutorial accessibility contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("desktop tutorial has no control root");
        }
        root.Measure(new Size(MinWidth, MinHeight));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > MinWidth
            || desired.Height > MinHeight)
        {
            throw new InvalidDataException(
                "desktop tutorial exceeded its minimum layout envelope");
        }
    }

    public void ProbeLocalizedPresentation()
    {
        var expected = DesktopTutorialCatalogs.Steps(localization);
        var originalStep = selectedStep;
        try
        {
            for (var index = 0; index < expected.Length; index++)
            {
                ShowStep(index);
                VerifyLocalizedPresentation(expected);
                VerifyLayoutEnvelope();
            }
        }
        finally
        {
            ShowStep(originalStep);
        }
    }

    private void VerifyLocalizedPresentation(DesktopTutorialStep[] expected)
    {
        var selected = expected[selectedStep];
        var finalStep = selectedStep == expected.Length - 1;
        var renderedPoints = lessonPoints.Children
            .OfType<Grid>()
            .Select(point => point.Children.OfType<TextBlock>().Last().Text)
            .ToArray();
        if (CurrentSteps.Length != 6
            || tutorialKicker.Text
                != localization.Text(DesktopTextKey.TutorialKicker)
            || tutorialHeading.Text
                != localization.Text(DesktopTextKey.TutorialHeading)
            || tutorialBody.Text != localization.Text(DesktopTextKey.TutorialBody)
            || lessonLabel.Text != selected.Label
            || lessonTitle.Text != selected.Title
            || lessonSummary.Text != selected.Summary
            || modelText.Text != selected.Model
            || !renderedPoints.SequenceEqual(selected.Points)
            || previousButton.Content as string
                != localization.Text(DesktopTextKey.Previous)
            || closeButton.Content as string
                != localization.Text(DesktopTextKey.Close)
            || AutomationProperties.GetName(progressText) != TutorialFormat(
                "a11y.progress.current",
                selectedStep + 1,
                CurrentSteps.Length,
                selected.Title)
            || AutomationProperties.GetHelpText(progressText)
                != TutorialText("help.progress")
            || AutomationProperties.GetName(previousButton)
                != TutorialText("a11y.previous")
            || AutomationProperties.GetHelpText(previousButton)
                != TutorialText("help.previous")
            || AutomationProperties.GetName(closeButton)
                != TutorialText("a11y.close")
            || AutomationProperties.GetHelpText(closeButton)
                != TutorialText("help.close")
            || nextButton.Content as string != localization.Text(
                finalStep ? DesktopTextKey.Finish : DesktopTextKey.Next)
            || AutomationProperties.GetName(nextButton) != TutorialText(
                finalStep ? "a11y.next.finish" : "a11y.next")
            || AutomationProperties.GetHelpText(nextButton) != TutorialText(
                finalStep ? "help.next.finish" : "help.next"))
        {
            throw new InvalidDataException(
                "localized desktop tutorial presentation drifted");
        }
        for (var index = 0; index < stepButtons.Count; index++)
        {
            var step = expected[index];
            var expectedName = TutorialFormat(
                index == selectedStep
                    ? "a11y.step.current"
                    : "a11y.step.open",
                index + 1,
                step.Title);
            if (AutomationProperties.GetName(stepButtons[index]) != expectedName
                || AutomationProperties.GetHelpText(stepButtons[index])
                    != TutorialFormat("help.step.jump", step.Label)
                || ToolTip.GetTip(stepButtons[index]) as string != step.Title)
            {
                throw new InvalidDataException(
                    "localized desktop tutorial step navigation drifted");
            }
        }
    }

    public void ProbeNavigationContract()
    {
        if (selectedStep != 0
            || previousButton.IsEnabled
            || nextButton.Content as string != localization.Text(DesktopTextKey.Next))
        {
            throw new InvalidDataException("desktop tutorial did not start at its first step");
        }
        nextButton.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        if (selectedStep != 1 || !previousButton.IsEnabled)
        {
            throw new InvalidDataException("desktop tutorial next control did not advance");
        }
        stepButtons[^1].RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        if (selectedStep != CurrentSteps.Length - 1
            || nextButton.Content as string != localization.Text(DesktopTextKey.Finish))
        {
            throw new InvalidDataException("desktop tutorial direct navigation did not reach the final step");
        }
        if (AutomationProperties.GetName(nextButton)
            != TutorialText("a11y.next.finish"))
        {
            throw new InvalidDataException(
                "desktop tutorial final action did not update its accessible meaning");
        }
        previousButton.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        stepButtons[0].RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        if (selectedStep != 0
            || previousButton.IsEnabled
            || progressText.Text != localization.Format(
                DesktopTextKey.StepProgress,
                1,
                CurrentSteps.Length))
        {
            throw new InvalidDataException("desktop tutorial navigation did not restore its first step");
        }
    }

    private void ConfigureNavigationControl(
        Control control,
        string automationId,
        string name,
        string helpText)
    {
        AutomationProperties.SetAutomationId(control, automationId);
        AutomationProperties.SetName(control, name);
        AutomationProperties.SetHelpText(control, helpText);
        auditedControls.Add(control);
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        ApplyLocalization();
    }

    private void ApplyLocalization()
    {
        currentSteps = DesktopTutorialCatalogs.Steps(localization);
        Title = $"Leserpent / {localization.Text(DesktopTextKey.LearningCenter).TrimEnd('.')}";
        FlowDirection = localization.FlowDirection;
        tutorialKicker.Text = localization.Text(DesktopTextKey.TutorialKicker);
        tutorialHeading.Text = localization.Text(DesktopTextKey.TutorialHeading);
        tutorialBody.Text = localization.Text(DesktopTextKey.TutorialBody);
        previousButton.Content = localization.Text(DesktopTextKey.Previous);
        closeButton.Content = localization.Text(DesktopTextKey.Close);
        AutomationProperties.SetHelpText(progressText, TutorialText("help.progress"));
        AutomationProperties.SetName(previousButton, TutorialText("a11y.previous"));
        AutomationProperties.SetHelpText(previousButton, TutorialText("help.previous"));
        AutomationProperties.SetName(closeButton, TutorialText("a11y.close"));
        AutomationProperties.SetHelpText(closeButton, TutorialText("help.close"));
        for (var index = 0; index < stepButtons.Count; index++)
        {
            ToolTip.SetTip(stepButtons[index], CurrentSteps[index].Title);
            AutomationProperties.SetHelpText(
                stepButtons[index],
                TutorialFormat("help.step.jump", CurrentSteps[index].Label));
        }
        ShowStep(Math.Min(selectedStep, CurrentSteps.Length - 1));
    }

    private void ShowStep(int index)
    {
        if (index < 0 || index >= CurrentSteps.Length)
        {
            return;
        }
        selectedStep = index;
        var step = CurrentSteps[index];
        lessonLabel.Text = step.Label;
        lessonTitle.Text = step.Title;
        lessonSummary.Text = step.Summary;
        modelText.Text = step.Model;
        lessonPoints.Children.Clear();
        foreach (var point in step.Points)
        {
            lessonPoints.Children.Add(Point(point));
        }
        for (var stepIndex = 0; stepIndex < stepButtons.Count; stepIndex++)
        {
            var selected = stepIndex == index;
            var button = stepButtons[stepIndex];
            button.Background = selected ? LeserpentTheme.Accent : LeserpentTheme.Panel;
            button.Foreground = selected ? Brushes.Black : LeserpentTheme.Primary;
            button.BorderBrush = selected ? LeserpentTheme.Accent : LeserpentTheme.PanelBorder;
            button.FontWeight = selected ? FontWeight.Bold : FontWeight.SemiBold;
            AutomationProperties.SetName(
                button,
                selected
                    ? TutorialFormat(
                        "a11y.step.current",
                        stepIndex + 1,
                        CurrentSteps[stepIndex].Title)
                    : TutorialFormat(
                        "a11y.step.open",
                        stepIndex + 1,
                        CurrentSteps[stepIndex].Title));
        }
        previousButton.IsEnabled = index > 0;
        var finalStep = index == CurrentSteps.Length - 1;
        nextButton.Content = finalStep
            ? localization.Text(DesktopTextKey.Finish)
            : localization.Text(DesktopTextKey.Next);
        AutomationProperties.SetName(
            nextButton,
            finalStep
                ? TutorialText("a11y.next.finish")
                : TutorialText("a11y.next"));
        AutomationProperties.SetHelpText(
            nextButton,
            finalStep
                ? TutorialText("help.next.finish")
                : TutorialText("help.next"));
        progressText.Text = localization.Format(
            DesktopTextKey.StepProgress,
            index + 1,
            CurrentSteps.Length);
        AutomationProperties.SetName(
            progressText,
            TutorialFormat(
                "a11y.progress.current",
                index + 1,
                CurrentSteps.Length,
                step.Title));
    }

    private string TutorialText(string key) =>
        DesktopTutorialCatalogs.Resolve(localization, key);

    private string TutorialFormat(string key, params object[] values) =>
        DesktopTutorialCatalogs.Format(localization, key, values);

    private void OnKeyDown(object? sender, KeyEventArgs eventArgs)
    {
        _ = sender;
        switch (eventArgs.Key)
        {
            case Key.Left:
                eventArgs.Handled = true;
                ShowStep(selectedStep - 1);
                break;
            case Key.Right when selectedStep < CurrentSteps.Length - 1:
                eventArgs.Handled = true;
                ShowStep(selectedStep + 1);
                break;
            case Key.Home:
                eventArgs.Handled = true;
                ShowStep(0);
                break;
            case Key.End:
                eventArgs.Handled = true;
                ShowStep(CurrentSteps.Length - 1);
                break;
            case Key.Escape:
                eventArgs.Handled = true;
                Close();
                break;
        }
    }

    private static Control Point(string text)
    {
        var body = new TextBlock
        {
            Text = text,
            Foreground = LeserpentTheme.Body,
            FontSize = 14,
            TextWrapping = TextWrapping.Wrap,
        };
        Grid.SetColumn(body, 1);
        return new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("Auto,*"),
            ColumnSpacing = 10,
            Children =
            {
                new TextBlock
                {
                    Text = "+",
                    Foreground = LeserpentTheme.Accent,
                    FontSize = 15,
                    FontWeight = FontWeight.Bold,
                },
                body,
            },
        };
    }

    private static bool ValidText(string value, int maxLength) =>
        value.Length is > 0
        && value.Length <= maxLength
        && value == value.Trim()
        && !value.Any(char.IsControl);
}
