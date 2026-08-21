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
    private static readonly DesktopTutorialStep[] EnglishSteps =
    [
        new(
            "SYSTEM MAP",
            "Read the topology",
            "Leserpent Desktop is a client, not the authority. It can manage many leserpentd authorities, and each daemon can own many Gewyvern runtime services.",
            [
                "The Hub root is this desktop operator session.",
                "Every daemon card is an independent local or remote authority with its own web service.",
                "Every runtime child stays routed through the daemon that owns it.",
            ],
            "Leserpent client -> leserpentd authority -> Gewyvern runtime"),
        new(
            "FIRST AUTHORITY",
            "Establish a daemon path",
            "Start locally or attach a remote machine without turning credentials into permanent application state.",
            [
                "Local Orchestra starts an app-owned loopback daemon on desktop systems.",
                "Deploy daemon uses supplied host credentials to install leserpentd remotely.",
                "Add daemon attaches an existing service using endpoint-bound trust and a runtime credential.",
                "Closing one daemon session does not close the Hub or another authority.",
            ],
            "Local Orchestra | Deploy daemon | + Add daemon"),
        new(
            "WORKSPACE",
            "Reach the right runtime",
            "Refresh topology before acting, then open a runtime beneath its owning daemon. A workspace never silently changes authority.",
            [
                "Refresh all joins existing work instead of starting duplicate requests.",
                "LIVE topology may open a workspace; retained or cached topology remains visibly stale.",
                "Provision gewyvern installs and registers a runtime through a selected daemon authority.",
                "A runtime button opens or focuses that runtime's native child window.",
            ],
            "Refresh topology -> choose authority -> open runtime"),
        new(
            "FIRST DIAGNOSIS",
            "Run a focused diagnostic",
            "Use a runtime workspace to deploy a typed pipeline, observe its bounded logs, and export diagnostics without losing daemon identity.",
            [
                "Choose a pipeline kind such as http/request instead of relying on an opaque button name.",
                "Add a target such as pid:4242 only when process scope is known.",
                "Inspect status, capabilities, snapshot changes, and severity before drawing a conclusion.",
                "Use explicit diagnostic export when another engineer or tool needs the evidence.",
            ],
            "pipeline=http/request  target=pid:4242"),
        new(
            "SAFETY FENCES",
            "Know when a change is blocked",
            "Leserpent fails closed when authority, freshness, capability, or revision evidence is missing. A disabled action is information, not friction to bypass.",
            [
                "Inspection and mutation availability come from shared policy, not frontend guesses.",
                "Deployment requires an authenticated capability and explicit confirmation.",
                "Revision drift or a closed workspace invalidates an in-progress submission.",
                "Unknown mutation outcomes require operator review and are never retried invisibly.",
            ],
            "live + authoritative + capable + confirmed -> mutate"),
        new(
            "LESELANG",
            "Automate the same interface",
            "Native controls and Leselang operations are two views of the same typed UI contract. Automation should not gain a hidden control plane.",
            [
                "Stable Automation IDs identify controls; business behavior comes from typed actions.",
                "Node IDs are opaque and must never be parsed as protocol commands.",
                "Leselang can focus, inspect, fill, submit, wait, and assert the same states a person sees.",
                "Export canonical Leselang before execution when reviewing or sharing an automated workflow.",
            ],
            "native control <-> typed UI action <-> Leselang"),
    ];

    private static readonly DesktopTutorialStep[] SimplifiedChineseSteps =
    [
        new(
            "系统地图",
            "读懂拓扑",
            "Leserpent Desktop 是客户端，而不是权威端。它可以管理多个 leserpentd 权威端，每个 daemon 又可以拥有多个 Gewyvern runtime 服务。",
            [
                "Hub 根节点代表当前桌面操作员会话。",
                "每张 daemon 卡片都是独立的本地或远程权威端，并拥有自己的 Web 服务。",
                "每个 runtime 子节点始终通过拥有它的 daemon 完成路由。",
            ],
            "Leserpent 客户端 -> leserpentd 权威端 -> Gewyvern runtime"),
        new(
            "首个权威端",
            "建立 daemon 路径",
            "可以从本机启动，也可以接入远程机器；主机凭证不会因此变成应用的永久状态。",
            [
                "在桌面系统上，本地 Orchestra 会启动由应用托管的回环 daemon。",
                "部署 daemon 会使用临时提供的主机凭证，在远程安装 leserpentd。",
                "添加 daemon 会通过端点绑定的信任与 runtime 凭证接入现有服务。",
                "关闭一个 daemon 会话不会关闭 Hub，也不会影响其他权威端。",
            ],
            "本地 Orchestra | 部署 daemon | + 添加 daemon"),
        new(
            "工作区",
            "抵达正确的 runtime",
            "执行操作前先刷新拓扑，再从所属 daemon 下打开 runtime。工作区绝不会在后台悄悄切换权威端。",
            [
                "全部刷新会加入已有任务，而不是重复发起请求。",
                "只有 LIVE 拓扑可以打开工作区；保留或缓存拓扑会持续显示为陈旧状态。",
                "部署 gewyvern 会通过选定的 daemon 权威端安装并注册 runtime。",
                "runtime 按钮会打开或聚焦该 runtime 的原生子窗口。",
            ],
            "刷新拓扑 -> 选择权威端 -> 打开 runtime"),
        new(
            "首次诊断",
            "执行聚焦诊断",
            "在 runtime 工作区部署类型化管道、观察有边界的日志并导出诊断，同时始终保留 daemon 身份。",
            [
                "选择 http/request 等管道类型，不要依赖含义不透明的按钮名称。",
                "只有明确进程范围时，才添加 pid:4242 这样的目标。",
                "得出结论前，检查状态、能力、快照变化和严重等级。",
                "其他工程师或工具需要证据时，使用显式诊断导出。",
            ],
            "pipeline=http/request  target=pid:4242"),
        new(
            "安全围栏",
            "理解操作为何被阻止",
            "缺少权威性、新鲜度、能力或 revision 证据时，Leserpent 会安全拒绝。禁用动作是在提供信息，不是应该绕过的阻力。",
            [
                "检查与变更可用性来自共享策略，而不是前端猜测。",
                "部署要求 authenticated capability（已认证能力）和明确确认。",
                "revision 漂移或工作区关闭会使正在提交的操作失效。",
                "未知变更结果必须由操作员复核，绝不会在后台隐式重试。",
            ],
            "live + authoritative + capable + confirmed -> mutate"),
        new(
            "LESELANG",
            "自动化同一套界面",
            "原生控件与 Leselang 操作是同一份类型化 UI 协议的两种视图；自动化不会获得隐藏的控制平面。",
            [
                "稳定的 Automation ID 标识控件，业务行为来自类型化动作。",
                "节点 ID 是不透明标识，绝不能解析成协议命令。",
                "Leselang 可以聚焦、检查、填写、提交、等待和断言人与机器看到的同一状态。",
                "复核或分享自动化流程时，在执行前导出规范化 Leselang。",
            ],
            "原生控件 <-> 类型化 UI 动作 <-> Leselang"),
    ];

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
        Content = "Previous",
        Padding = new Thickness(16, 8),
    };
    private readonly Button nextButton = new()
    {
        Content = "Next",
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(20, 8),
    };
    private readonly Button closeButton = new()
    {
        Content = "Close",
        Padding = new Thickness(16, 8),
    };
    private int selectedStep;
    private DesktopTutorialStep[] CurrentSteps => localization.Active.Locale == "zh-CN"
        ? SimplifiedChineseSteps
        : EnglishSteps;

    public DesktopTutorialWindow(DesktopLocalization? localization = null)
    {
        this.localization = localization ?? DesktopLocalization.ForVerification();
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
            "Tutorial progress",
            "Announces the current tutorial step.");
        AutomationProperties.SetLiveSetting(progressText, AutomationLiveSetting.Polite);
        ConfigureNavigationControl(
            previousButton,
            "desktop-tutorial-previous",
            "Open the previous tutorial step",
            "Shortcut: Left Arrow.");
        ConfigureNavigationControl(
            nextButton,
            "desktop-tutorial-next",
            "Open the next tutorial step",
            "Advances until the final step. Shortcut: Right Arrow.");
        ConfigureNavigationControl(
            closeButton,
            "desktop-tutorial-close",
            "Close the Leserpent tutorial",
            "Returns to the Hub without starting an operation. Shortcut: Escape.");
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
                $"Open tutorial step {index + 1}: {CurrentSteps[index].Title}",
                $"Jumps directly to {CurrentSteps[index].Label.ToLowerInvariant()}.");
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
        foreach (var steps in new[] { EnglishSteps, SimplifiedChineseSteps })
        {
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
            EnglishSteps.SelectMany(step => step.Points.Prepend(step.Summary)));
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
        if (AutomationProperties.GetName(nextButton) != "Finish and close the Leserpent tutorial")
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
        Title = $"Leserpent / {localization.Text(DesktopTextKey.LearningCenter).TrimEnd('.')}";
        FlowDirection = localization.FlowDirection;
        tutorialKicker.Text = localization.Text(DesktopTextKey.TutorialKicker);
        tutorialHeading.Text = localization.Text(DesktopTextKey.TutorialHeading);
        tutorialBody.Text = localization.Text(DesktopTextKey.TutorialBody);
        previousButton.Content = localization.Text(DesktopTextKey.Previous);
        closeButton.Content = localization.Text(DesktopTextKey.Close);
        AutomationProperties.SetName(previousButton, localization.Text(DesktopTextKey.Previous));
        AutomationProperties.SetName(closeButton, localization.Text(DesktopTextKey.Close));
        for (var index = 0; index < stepButtons.Count; index++)
        {
            ToolTip.SetTip(stepButtons[index], CurrentSteps[index].Title);
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
                    ? $"Current tutorial step {stepIndex + 1}: {CurrentSteps[stepIndex].Title}"
                    : $"Open tutorial step {stepIndex + 1}: {CurrentSteps[stepIndex].Title}");
        }
        previousButton.IsEnabled = index > 0;
        var finalStep = index == CurrentSteps.Length - 1;
        nextButton.Content = finalStep
            ? localization.Text(DesktopTextKey.Finish)
            : localization.Text(DesktopTextKey.Next);
        AutomationProperties.SetName(
            nextButton,
            finalStep
                ? "Finish and close the Leserpent tutorial"
                : "Open the next tutorial step");
        AutomationProperties.SetHelpText(
            nextButton,
            finalStep
                ? "Closes the tutorial and returns to the Hub without starting an operation."
                : "Advances until the final step. Shortcut: Right Arrow.");
        progressText.Text = localization.Format(
            DesktopTextKey.StepProgress,
            index + 1,
            CurrentSteps.Length);
        AutomationProperties.SetName(
            progressText,
            $"Tutorial step {index + 1} of {CurrentSteps.Length}: {step.Title}");
    }

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
