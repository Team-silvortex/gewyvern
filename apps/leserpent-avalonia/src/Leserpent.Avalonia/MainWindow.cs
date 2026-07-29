using System.Text.Json;
using System.Text.Json.Nodes;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class MainWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly Task<PresentationAutomationResult> initialRealizedWait;
    private readonly Task<PresentationAutomationResult> initialRealizedWaitTimeout;
    private readonly Task<PresentationAutomationResult> initialVisibleWait;
    private readonly Task<PresentationAutomationResult> initialVisibleWaitTimeout;
    private readonly Task<PresentationAutomationResult> initialEnabledWait;
    private readonly Task<PresentationAutomationResult> initialEnabledWaitTimeout;
    private readonly Task<PresentationAutomationResult> initialFocusedWait;
    private readonly Task<PresentationAutomationResult> initialFocusedWaitTimeout;
    private readonly Task<PresentationAutomationResult> initialSelectionWait;
    private readonly Task<PresentationAutomationResult> initialSelectionWaitTimeout;
    private readonly Task<PresentationAutomationResult> initialWindowOpenWait;
    private readonly string initialFocusedWaitNodeId;
    private readonly string initialFocusedWaitTimeoutNodeId;
    private readonly string initialSelectionAssertNodeId;
    private readonly string initialSelectionWaitNodeId;
    private readonly string initialWindowOpenAssertNodeId;
    private readonly string initialWindowOpenWaitNodeId;
    private int invokedActionCount;
    private readonly TextBlock statusText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 13,
        Text = "No action selected",
    };

    public int RenderedNodeCount { get; }
    public int AppliedPatchOperations { get; }
    public int ReusedNodeCount { get; }
    public int VirtualizedHostCount { get; }
    public int ActiveVirtualizedHostCount => renderer.ActiveVirtualizedHostCount;
    public int InitialUnrealizedVirtualItemCount { get; }
    public int UnrealizedVirtualItemCount => renderer.UnrealizedVirtualItemCount;
    public int InitialUnrealizedNodeCount { get; }
    public int UnrealizedNodeCount => renderer.UnrealizedNodeCount;
    public bool InitialUnrealizedAssertionRejected { get; }
    public bool InitialUnrealizedNavigationRejected { get; }
    public bool InitialRealizedWaitCompleted { get; private set; }
    public bool InitialRealizedWaitTimedOut { get; private set; }
    public bool InitialVisibleWaitCompleted { get; private set; }
    public bool InitialVisibleWaitTimedOut { get; private set; }
    public bool InitialHiddenWaitCompleted { get; private set; }
    public bool InitialHiddenWaitTimedOut { get; private set; }
    public bool InitialEnabledWaitCompleted { get; private set; }
    public bool InitialEnabledWaitTimedOut { get; private set; }
    public bool InitialDisabledWaitCompleted { get; private set; }
    public bool InitialDisabledWaitTimedOut { get; private set; }
    public bool WindowOpenAssertCompleted { get; private set; }
    public bool WindowOpenWaitCompleted { get; private set; }
    public bool InitialFocusedWaitCompleted { get; private set; }
    public bool InitialFocusedWaitTimedOut { get; private set; }
    public bool InitialSelectionWaitCompleted { get; private set; }
    public bool InitialSelectionWaitTimedOut { get; private set; }
    public bool SelectionAssertCompleted { get; private set; }
    public bool SelectionMismatchRejected { get; private set; }
    public bool SelectionlessTargetRejected { get; private set; }
    public bool SelectionProbePreservedFocus { get; private set; }
    public bool FocusNavigationForwardCompleted { get; private set; }
    public bool FocusNavigationBackwardCompleted { get; private set; }
    public bool FocusNavigationFirstCompleted { get; private set; }
    public bool FocusNavigationLastCompleted { get; private set; }
    public bool FocusNavigationFailuresPreservedFocus { get; private set; }
    public bool FocusNavigationDidNotActivate { get; private set; }
    public bool ActionKindAssertCompleted { get; private set; }
    public bool ActionKindMismatchRejected { get; private set; }
    public bool FormFieldAssertCompleted { get; private set; }
    public bool FormFieldMismatchRejected { get; private set; }
    public bool FormFieldInputKindAssertCompleted { get; private set; }
    public bool FormFieldInputKindMismatchRejected { get; private set; }
    public bool FormFieldRequiredAssertCompleted { get; private set; }
    public bool FormFieldRequiredMismatchRejected { get; private set; }
    public bool DisabledAssertCompleted { get; private set; }
    public bool DisabledMismatchRejected { get; private set; }
    public bool HiddenAssertCompleted { get; private set; }
    public bool VisibleMismatchRejected { get; private set; }
    public int InitialDebuggerCancelButtonCount { get; }
    public int DebuggerCancelButtonCount => renderer.RealizedDebuggerCancelButtonCount;
    public int DisabledActionProbeCount { get; private set; }
    public AccessibilityAudit InitialAccessibility { get; }
    public AccessibilityAudit Accessibility => renderer.AuditAccessibility();
    public ulong Revision { get; }

    public MainWindow(RendererFixture fixture)
    {
        Width = 1080;
        Height = 760;
        MinWidth = 640;
        MinHeight = 480;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        renderer = new AvaloniaDocumentRenderer(OnActionInvoked);
        renderer.Mount(fixture.Previous);
        InitialDebuggerCancelButtonCount = renderer.RealizedDebuggerCancelButtonCount;
        InitialAccessibility = renderer.AuditAccessibility();
        renderer.Apply(fixture.Patch);
        RequireExpectedDocument(renderer.Document, fixture.Next);
        _ = renderer.AuditAccessibility();

        Revision = renderer.Document.Revision;
        RenderedNodeCount = renderer.NodeCount;
        AppliedPatchOperations = renderer.LastAppliedOperationCount;
        ReusedNodeCount = renderer.LastReusedNodeCount;
        VirtualizedHostCount = renderer.VirtualizedHostCount;
        InitialUnrealizedVirtualItemCount = renderer.UnrealizedVirtualItemCount;
        InitialUnrealizedNodeCount = renderer.UnrealizedNodeCount;
        var initialUnrealizedActionNodeId = renderer.FirstUnrealizedActionNodeId
            ?? throw new InvalidDataException(
                "focus navigation probe requires a pre-layout unrealized action");
        var initialUnrealizedNavigation = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.NavigateFocus,
                NodeId = initialUnrealizedActionNodeId,
                Direction = UiFocusNavigationDirection.Next,
            });
        InitialUnrealizedNavigationRejected =
            !initialUnrealizedNavigation.Applied
            && initialUnrealizedNavigation.FailureCode
                == PresentationAutomationFailureCode.TargetUnrealized;
        if (!InitialUnrealizedNavigationRejected)
        {
            throw new InvalidDataException(
                "Leselang focus navigation accepted a pre-layout unrealized action");
        }
        var initialUnrealizedNodeId = renderer.FirstUnrealizedNodeId
            ?? throw new InvalidDataException(
                "realization assertion probe requires a pre-layout virtualized semantic node");
        var initialUnrealized = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertRealized,
            NodeId = initialUnrealizedNodeId,
        });
        InitialUnrealizedAssertionRejected = !initialUnrealized.Applied
            && initialUnrealized.FailureCode
                == PresentationAutomationFailureCode.TargetUnrealized;
        if (!InitialUnrealizedAssertionRejected)
        {
            throw new InvalidDataException(
                "Leselang realization assertion accepted a pre-layout unrealized target");
        }
        initialRealizedWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitRealized,
            NodeId = initialUnrealizedNodeId,
            TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs,
        });
        var visibleWaitNodeId = fixture.VisibleWaitOperation?.NodeId
            ?? throw new InvalidDataException(
                "visibility wait probe requires a semantic target");
        initialVisibleWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitVisible,
            NodeId = visibleWaitNodeId,
            TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
        });
        var enabledWaitNodeId = fixture.EnabledWaitOperation?.NodeId
            ?? throw new InvalidDataException(
                "enabled wait probe requires an action target");
        renderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            false,
            "Verification action is temporarily unavailable");
        initialEnabledWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitEnabled,
            NodeId = enabledWaitNodeId,
            TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
        });
        var detachedRenderer = new AvaloniaDocumentRenderer(_ => { });
        detachedRenderer.Mount(fixture.Next);
        detachedRenderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            false,
            "Verification action remains unavailable");
        var detachedUnrealizedNodeId = detachedRenderer.FirstUnrealizedNodeId
            ?? throw new InvalidDataException(
                "realization wait timeout probe requires a detached unrealized node");
        initialRealizedWaitTimeout = detachedRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitRealized,
                NodeId = detachedUnrealizedNodeId,
                TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs,
            });
        initialVisibleWaitTimeout = detachedRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitVisible,
                NodeId = visibleWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
            });
        initialEnabledWaitTimeout = detachedRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitEnabled,
                NodeId = enabledWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
            });
        initialFocusedWaitNodeId = fixture.FocusedWaitOperation?.NodeId
            ?? throw new InvalidDataException(
                "focused wait probe requires an action target");
        initialFocusedWaitTimeoutNodeId = FindOtherActionNodeId(
                renderer.Document.Root,
                initialFocusedWaitNodeId)
            ?? throw new InvalidDataException(
                "focused wait timeout probe requires another action target");
        initialFocusedWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFocused,
                NodeId = initialFocusedWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitFocusedTimeoutMs,
            });
        initialFocusedWaitTimeout = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFocused,
                NodeId = initialFocusedWaitTimeoutNodeId,
                TimeoutMs = SemanticRenderer.WaitFocusedTimeoutMs,
            });
        initialSelectionAssertNodeId = fixture.SelectionAssertOperation?.NodeId
            ?? throw new InvalidDataException(
                "selection assertion probe requires a selectable target");
        initialSelectionWaitNodeId = fixture.SelectionWaitOperation?.NodeId
            ?? throw new InvalidDataException(
                "selection wait probe requires a selectable target");
        initialSelectionWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitSelection,
                NodeId = initialSelectionWaitNodeId,
                State = UiSelectionState.Unselected,
                TimeoutMs = SemanticRenderer.WaitSelectionTimeoutMs,
            });
        initialWindowOpenAssertNodeId = fixture.WindowOpenAssertOperation?.NodeId
            ?? throw new InvalidDataException(
                "window-open assertion probe requires a semantic target");
        initialWindowOpenWaitNodeId = fixture.WindowOpenWaitOperation?.NodeId
            ?? throw new InvalidDataException(
                "window-open wait probe requires a semantic target");
        initialWindowOpenWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitWindowOpen,
                NodeId = initialWindowOpenWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitWindowOpenTimeoutMs,
            });
        initialSelectionWaitTimeout = detachedRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitSelection,
                NodeId = initialSelectionWaitNodeId,
                State = UiSelectionState.Selected,
                TimeoutMs = SemanticRenderer.WaitSelectionTimeoutMs,
            });
        Title = $"Leserpent / revision {Revision}";
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto"),
            Children =
            {
                new Border
                {
                    Padding = new Thickness(32, 28),
                    Child = renderer.Surface,
                },
                BuildStatusBar(),
            },
        };
    }

    public async Task CompleteInitialWaitProbesAsync()
    {
        var windowOpen = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertWindowOpen,
            NodeId = initialWindowOpenAssertNodeId,
        });
        WindowOpenAssertCompleted = windowOpen.Applied
            && windowOpen.FailureCode == PresentationAutomationFailureCode.None;
        if (!WindowOpenAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang window-open assertion did not observe the native window");
        }
        var windowOpenWait = await initialWindowOpenWait;
        WindowOpenWaitCompleted = windowOpenWait.Applied
            && windowOpenWait.FailureCode == PresentationAutomationFailureCode.None;
        if (!WindowOpenWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang window-open wait did not observe the native window");
        }
        DispatcherTimer.RunOnce(
            () => renderer.SetActionAvailability(
                ActionKind.RuntimeRefresh,
                true,
                null),
            TimeSpan.FromMilliseconds(50));
        DispatcherTimer.RunOnce(
            () =>
            {
                var focused = renderer.ApplyPresentation(new UiPresentationOperation
                {
                    Kind = UiPresentationOperationKind.Focus,
                    NodeId = initialFocusedWaitNodeId,
                });
                if (!focused.Applied
                    || focused.FailureCode
                        != PresentationAutomationFailureCode.None)
                {
                    throw new InvalidDataException(
                        "focused wait probe could not apply its external focus transition");
                }
            },
            TimeSpan.FromMilliseconds(50));
        var result = await initialRealizedWait;
        InitialRealizedWaitCompleted = result.Applied
            && result.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialRealizedWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang realization wait did not observe natural post-layout realization");
        }
        var timeoutResult = await initialRealizedWaitTimeout;
        InitialRealizedWaitTimedOut = !timeoutResult.Applied
            && timeoutResult.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        if (!InitialRealizedWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang realization wait did not reject a persistently unrealized target");
        }
        var visibleResult = await initialVisibleWait;
        InitialVisibleWaitCompleted = visibleResult.Applied
            && visibleResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialVisibleWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang visibility wait did not observe natural post-layout visibility");
        }
        var visibleTimeoutResult = await initialVisibleWaitTimeout;
        InitialVisibleWaitTimedOut = !visibleTimeoutResult.Applied
            && visibleTimeoutResult.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        if (!InitialVisibleWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang visibility wait did not reject a persistently invisible target");
        }
        var enabledResult = await initialEnabledWait;
        InitialEnabledWaitCompleted = enabledResult.Applied
            && enabledResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialEnabledWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang enabled wait did not observe an external availability transition");
        }
        var enabledTimeoutResult = await initialEnabledWaitTimeout;
        InitialEnabledWaitTimedOut = !enabledTimeoutResult.Applied
            && enabledTimeoutResult.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        if (!InitialEnabledWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang enabled wait did not reject a persistently disabled target");
        }
        var disabledWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitDisabled,
            NodeId = enabledResult.NodeId,
            TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => renderer.SetActionAvailability(
                ActionKind.RuntimeRefresh,
                false,
                "Verification action is temporarily unavailable"),
            TimeSpan.FromMilliseconds(50));
        var disabledWaitResult = await disabledWait;
        InitialDisabledWaitCompleted = disabledWaitResult.Applied
            && disabledWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, true, null);
        if (!InitialDisabledWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang disabled wait did not observe an external availability transition");
        }
        var restoredAvailability = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertEnabled,
            NodeId = enabledResult.NodeId,
        });
        if (!restoredAvailability.Applied
            || restoredAvailability.FailureCode != PresentationAutomationFailureCode.None)
        {
            throw new InvalidDataException(
                "Leselang disabled wait did not restore the enabled probe target");
        }
        var disabledTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitDisabled,
                NodeId = enabledResult.NodeId,
                TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
            });
        InitialDisabledWaitTimedOut = !disabledTimeoutResult.Applied
            && disabledTimeoutResult.FailureCode == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertEnabled,
                NodeId = enabledResult.NodeId,
            }).Applied;
        if (!InitialDisabledWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang disabled wait did not reject a persistently enabled target");
        }
        var focusedResult = await initialFocusedWait;
        InitialFocusedWaitCompleted = focusedResult.Applied
            && focusedResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialFocusedWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang focused wait did not observe an external focus transition");
        }
        var focusedTimeoutResult = await initialFocusedWaitTimeout;
        var timeoutTargetRealized = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertRealized,
                NodeId = initialFocusedWaitTimeoutNodeId,
            });
        var timeoutTargetFocused = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertFocused,
                NodeId = initialFocusedWaitTimeoutNodeId,
            });
        InitialFocusedWaitTimedOut = !focusedTimeoutResult.Applied
            && focusedTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && timeoutTargetRealized.Applied
            && timeoutTargetRealized.FailureCode
                == PresentationAutomationFailureCode.None
            && !timeoutTargetFocused.Applied
            && timeoutTargetFocused.FailureCode
                == PresentationAutomationFailureCode.TargetNotFocused
            && renderer.FocusedNodeId == initialFocusedWaitNodeId;
        if (!InitialFocusedWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang focused wait changed focus or did not reject a persistently unfocused realized target");
        }
        var selectionResult = await initialSelectionWait;
        InitialSelectionWaitCompleted = selectionResult.Applied
            && selectionResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialSelectionWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang selection wait did not observe native unselected state");
        }
        var selectionTimeoutResult = await initialSelectionWaitTimeout;
        InitialSelectionWaitTimedOut = !selectionTimeoutResult.Applied
            && selectionTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut;
        if (!InitialSelectionWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang selection wait did not reject a persistently unmatched selection state");
        }
        var hiddenWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitHidden,
            NodeId = visibleResult.NodeId,
            TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => renderer.Surface.IsVisible = false,
            TimeSpan.FromMilliseconds(50));
        var hiddenWaitResult = await hiddenWait;
        InitialHiddenWaitCompleted = hiddenWaitResult.Applied
            && hiddenWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        renderer.Surface.IsVisible = true;
        if (!InitialHiddenWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang hidden wait did not observe an external hidden transition");
        }
        var restoredVisibility = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertVisible,
            NodeId = visibleResult.NodeId,
        });
        if (!restoredVisibility.Applied
            || restoredVisibility.FailureCode != PresentationAutomationFailureCode.None)
        {
            throw new InvalidDataException(
                "Leselang hidden wait did not restore the visible probe target");
        }
        var hiddenTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitHidden,
                NodeId = visibleResult.NodeId,
                TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
            });
        InitialHiddenWaitTimedOut = !hiddenTimeoutResult.Applied
            && hiddenTimeoutResult.FailureCode == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertVisible,
                NodeId = visibleResult.NodeId,
            }).Applied;
        if (!InitialHiddenWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang hidden wait did not reject a persistently visible target");
        }
    }

    public void ProbeActionAvailability()
    {
        ActionKind[] mutationKinds =
        [
            ActionKind.RuntimeRefresh,
            ActionKind.RuntimeCapabilitiesRefresh,
            ActionKind.RuntimeDeploy,
        ];
        var realizedActions = mutationKinds.Sum(renderer.RealizedActionCount);
        if (realizedActions == 0)
        {
            DisabledActionProbeCount = 0;
            _ = renderer.AuditAccessibility();
            return;
        }
        foreach (var kind in mutationKinds)
        {
            renderer.SetActionAvailability(
                kind,
                false,
                "Verification action is temporarily unavailable");
        }
        DisabledActionProbeCount = mutationKinds.Sum(
            renderer.RealizedDisabledActionCount);
        if (DisabledActionProbeCount != realizedActions)
        {
            throw new InvalidDataException(
                "Avalonia action availability did not disable every realized mutation action");
        }
        foreach (var kind in mutationKinds)
        {
            renderer.SetActionAvailability(kind, true, null);
        }
        if (mutationKinds.Sum(renderer.RealizedDisabledActionCount) != 0)
        {
            throw new InvalidDataException(
                "Avalonia action availability did not restore every realized mutation action");
        }
        _ = renderer.AuditAccessibility();
    }

    public string BeginFocusRetentionProbe()
    {
        var nodeId = renderer.FirstRealizedActionNodeId
            ?? throw new InvalidDataException("focus probe requires a realized action");
        var applied = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = nodeId,
        });
        if (!applied.Applied
            || applied.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang presentation focus could not focus its action node");
        }
        var focused = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFocused,
            NodeId = nodeId,
        });
        if (!focused.Applied
            || focused.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang focus assertion rejected the focused action");
        }
        var actionCountBeforeNavigation = invokedActionCount;
        var navigatedForward = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.NavigateFocus,
            NodeId = nodeId,
            Direction = UiFocusNavigationDirection.Next,
        });
        if (!navigatedForward.Applied
            || navigatedForward.FailureCode != PresentationAutomationFailureCode.None
            || navigatedForward.FocusedNodeId is not { } forwardNodeId
            || forwardNodeId == nodeId
            || renderer.FocusedNodeId != forwardNodeId)
        {
            throw new InvalidDataException(
                "Leselang native forward focus navigation did not return its stable destination");
        }
        FocusNavigationForwardCompleted = true;
        var navigatedBackward = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.NavigateFocus,
            NodeId = forwardNodeId,
            Direction = UiFocusNavigationDirection.Previous,
        });
        if (!navigatedBackward.Applied
            || navigatedBackward.FailureCode != PresentationAutomationFailureCode.None
            || navigatedBackward.FocusedNodeId is not { } backwardNodeId
            || backwardNodeId == forwardNodeId
            || renderer.FocusedNodeId != backwardNodeId)
        {
            throw new InvalidDataException(
                "Leselang native backward focus navigation did not return its stable destination");
        }
        FocusNavigationBackwardCompleted = true;
        var navigatedLast = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.NavigateFocus,
            NodeId = backwardNodeId,
            Direction = UiFocusNavigationDirection.Last,
        });
        if (!navigatedLast.Applied
            || navigatedLast.FailureCode != PresentationAutomationFailureCode.None
            || navigatedLast.FocusedNodeId is not { } lastNodeId
            || lastNodeId == backwardNodeId
            || renderer.FocusedNodeId != lastNodeId)
        {
            throw new InvalidDataException(
                $"Leselang native last focus navigation did not return its stable destination: applied={navigatedLast.Applied}, failure={navigatedLast.FailureCode}, result={navigatedLast.FocusedNodeId ?? "none"}, focused={renderer.FocusedNodeId ?? "none"}");
        }
        FocusNavigationLastCompleted = true;
        var navigatedFirst = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.NavigateFocus,
            NodeId = lastNodeId,
            Direction = UiFocusNavigationDirection.First,
        });
        if (!navigatedFirst.Applied
            || navigatedFirst.FailureCode != PresentationAutomationFailureCode.None
            || navigatedFirst.FocusedNodeId is not { } firstNodeId
            || firstNodeId == lastNodeId
            || renderer.FocusedNodeId != firstNodeId)
        {
            throw new InvalidDataException(
                $"Leselang native first focus navigation did not return its stable destination: applied={navigatedFirst.Applied}, failure={navigatedFirst.FailureCode}, result={navigatedFirst.FocusedNodeId ?? "none"}, focused={renderer.FocusedNodeId ?? "none"}");
        }
        FocusNavigationFirstCompleted = true;
        if (!renderer.TryFocusNode(nodeId) || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "focus navigation probe could not restore its original action");
        }
        FocusNavigationDidNotActivate = invokedActionCount == actionCountBeforeNavigation;
        if (!FocusNavigationDidNotActivate)
        {
            throw new InvalidDataException(
                "Leselang focus navigation activated an action");
        }
        var otherActionNodeId = FindOtherActionNodeId(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException("focus assertion probe requires another action");
        var unfocused = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFocused,
            NodeId = otherActionNodeId,
        });
        if (unfocused.Applied
            || unfocused.FailureCode != PresentationAutomationFailureCode.TargetNotFocused
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang focus assertion accepted an unfocused action or changed focus");
        }
        var unfocusedNavigation = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.NavigateFocus,
            NodeId = otherActionNodeId,
            Direction = UiFocusNavigationDirection.Previous,
        });
        if (unfocusedNavigation.Applied
            || unfocusedNavigation.FailureCode
                != PresentationAutomationFailureCode.TargetNotFocused
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang focus navigation accepted an unfocused source or changed focus");
        }
        var enabledNodeId = renderer.FirstRealizedActionNodeIdFor(ActionKind.RuntimeRefresh)
            ?? throw new InvalidDataException("enabled assertion probe requires a refresh action");
        var enabled = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertEnabled,
            NodeId = enabledNodeId,
        });
        if (!enabled.Applied
            || enabled.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang enabled assertion rejected an enabled action or changed focus");
        }
        renderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            false,
            "Verification action is temporarily unavailable");
        var disabled = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertEnabled,
            NodeId = enabledNodeId,
        });
        var disabledAssert = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertDisabled,
            NodeId = enabledNodeId,
        });
        DisabledAssertCompleted = disabledAssert.Applied
            && disabledAssert.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, true, null);
        var disabledMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertDisabled,
            NodeId = enabledNodeId,
        });
        DisabledMismatchRejected = !disabledMismatch.Applied
            && disabledMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetStillEnabled
            && renderer.FocusedNodeId == nodeId;
        if (disabled.Applied
            || disabled.FailureCode != PresentationAutomationFailureCode.TargetNotEnabled
            || !DisabledAssertCompleted
            || !DisabledMismatchRejected
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang enabled/disabled assertion accepted the wrong action state or changed focus");
        }
        var textNode = FindFirstTextNode(renderer.Document.Root)
            ?? throw new InvalidDataException("text assertion probe requires a text leaf");
        var expectedText = textNode.Text?.Fallback
            ?? throw new InvalidDataException("text assertion target has no semantic text");
        var textMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertText,
            NodeId = textNode.Id,
            Expected = expectedText,
        });
        if (!textMatched.Applied
            || textMatched.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang text assertion rejected native display text or changed focus");
        }
        var textMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertText,
            NodeId = textNode.Id,
            Expected = $"{expectedText} mismatch",
        });
        if (textMismatch.Applied
            || textMismatch.FailureCode != PresentationAutomationFailureCode.TargetTextMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang text assertion accepted mismatched native display text or changed focus");
        }
        var automationIdMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAutomationId,
            NodeId = textNode.Id,
            Expected = textNode.Id,
        });
        if (!automationIdMatched.Applied
            || automationIdMatched.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang automation id assertion rejected native automation identity or changed focus");
        }
        var automationIdMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAutomationId,
            NodeId = textNode.Id,
            Expected = $"{textNode.Id}-mismatch",
        });
        if (automationIdMismatch.Applied
            || automationIdMismatch.FailureCode
                != PresentationAutomationFailureCode.TargetAutomationIdMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang automation id assertion accepted mismatched native identity or changed focus");
        }
        var nodeKindMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertNodeKind,
            NodeId = textNode.Id,
            ExpectedKind = textNode.Kind,
        });
        if (!nodeKindMatched.Applied
            || nodeKindMatched.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang node kind assertion rejected the stable semantic renderer kind or changed focus");
        }
        var mismatchedNodeKind = textNode.Kind == UiNodeKind.Text
            ? UiNodeKind.Heading
            : UiNodeKind.Text;
        var nodeKindMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertNodeKind,
            NodeId = textNode.Id,
            ExpectedKind = mismatchedNodeKind,
        });
        if (nodeKindMismatch.Applied
            || nodeKindMismatch.FailureCode
                != PresentationAutomationFailureCode.TargetNodeKindMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang node kind assertion accepted a mismatched semantic renderer kind or changed focus");
        }
        var focusedActionNode = FindNode(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException("action kind probe target was not found");
        var expectedActionKind = focusedActionNode.Action?.Kind
            ?? throw new InvalidDataException("action kind probe requires an action target");
        var actionKindMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionKind,
            NodeId = nodeId,
            ExpectedActionKind = expectedActionKind,
        });
        ActionKindAssertCompleted = actionKindMatched.Applied
            && actionKindMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!ActionKindAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang action kind assertion rejected the stable semantic action kind or changed focus");
        }
        var actionKindMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionKind,
            NodeId = nodeId,
            ExpectedActionKind = MismatchedActionKind(expectedActionKind),
        });
        ActionKindMismatchRejected = !actionKindMismatch.Applied
            && actionKindMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetActionKindMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!ActionKindMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang action kind assertion accepted a mismatched semantic action kind or changed focus");
        }
        var formActionNodeId = renderer.FirstRealizedActionNodeIdFor(ActionKind.RuntimeDeploy)
            ?? throw new InvalidDataException(
                "form field assertion probe requires a realized deployment form action");
        var formActionNode = FindNode(renderer.Document.Root, formActionNodeId)
            ?? throw new InvalidDataException("form field assertion probe target was not found");
        var formField = formActionNode.Action?.Form?.Fields.FirstOrDefault()
            ?? throw new InvalidDataException(
                "form field assertion probe requires form field metadata");
        var expectedFormFieldLabel = formField.Label.Fallback;
        var formFieldMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormField,
            NodeId = formActionNodeId,
            Field = formField.Key,
            Expected = expectedFormFieldLabel,
        });
        FormFieldAssertCompleted = formFieldMatched.Applied
            && formFieldMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field assertion rejected stable form metadata or changed focus");
        }
        var formFieldMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormField,
            NodeId = formActionNodeId,
            Field = formField.Key,
            Expected = $"{expectedFormFieldLabel} mismatch",
        });
        FormFieldMismatchRejected = !formFieldMismatch.Applied
            && formFieldMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetFormFieldMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang form field assertion accepted mismatched form metadata or changed focus");
        }
        var formFieldInputKindMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
            NodeId = formActionNodeId,
            Field = formField.Key,
            InputKind = formField.InputKind,
        });
        FormFieldInputKindAssertCompleted = formFieldInputKindMatched.Applied
            && formFieldInputKindMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldInputKindAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field input kind assertion rejected stable form metadata or changed focus");
        }
        var formFieldInputKindMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
            NodeId = formActionNodeId,
            Field = formField.Key,
            InputKind = MismatchedInputKind(formField.InputKind),
        });
        FormFieldInputKindMismatchRejected = !formFieldInputKindMismatch.Applied
            && formFieldInputKindMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetFormFieldInputKindMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldInputKindMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang form field input kind assertion accepted mismatched form metadata or changed focus");
        }
        var formFieldRequiredMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldRequired,
            NodeId = formActionNodeId,
            Field = formField.Key,
            Required = formField.Required,
        });
        FormFieldRequiredAssertCompleted = formFieldRequiredMatched.Applied
            && formFieldRequiredMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldRequiredAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field required assertion rejected stable form metadata or changed focus");
        }
        var formFieldRequiredMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldRequired,
            NodeId = formActionNodeId,
            Field = formField.Key,
            Required = !formField.Required,
        });
        FormFieldRequiredMismatchRejected = !formFieldRequiredMismatch.Applied
            && formFieldRequiredMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetFormFieldRequiredMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldRequiredMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang form field required assertion accepted mismatched form metadata or changed focus");
        }
        var expectedAccessibleName = textNode.Accessibility.Label?.Fallback
            ?? textNode.Text?.Fallback
            ?? textNode.Id;
        var accessibleNameMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAccessibleName,
            NodeId = textNode.Id,
            Expected = expectedAccessibleName,
        });
        if (!accessibleNameMatched.Applied
            || accessibleNameMatched.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible name assertion rejected native automation metadata or changed focus");
        }
        var accessibleNameMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAccessibleName,
            NodeId = textNode.Id,
            Expected = $"{expectedAccessibleName} mismatch",
        });
        if (accessibleNameMismatch.Applied
            || accessibleNameMismatch.FailureCode
                != PresentationAutomationFailureCode.TargetAccessibleNameMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible name assertion accepted mismatched native metadata or changed focus");
        }
        var descriptionNode = FindFirstDescriptionNode(renderer.Document.Root)
            ?? throw new InvalidDataException(
                "accessible description probe requires explicit semantic metadata");
        var expectedAccessibleDescription = descriptionNode.Accessibility.Description!.Fallback;
        var accessibleDescriptionMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAccessibleDescription,
            NodeId = descriptionNode.Id,
            Expected = expectedAccessibleDescription,
        });
        if (!accessibleDescriptionMatched.Applied
            || accessibleDescriptionMatched.FailureCode
                != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible description assertion rejected native automation metadata or changed focus");
        }
        var accessibleDescriptionMismatch = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertAccessibleDescription,
                NodeId = descriptionNode.Id,
                Expected = $"{expectedAccessibleDescription} mismatch",
            });
        if (accessibleDescriptionMismatch.Applied
            || accessibleDescriptionMismatch.FailureCode
                != PresentationAutomationFailureCode.TargetAccessibleDescriptionMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible description assertion accepted mismatched native metadata or changed focus");
        }
        var selectionMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertSelection,
            NodeId = initialSelectionAssertNodeId,
            State = UiSelectionState.Selected,
        });
        SelectionAssertCompleted = selectionMatched.Applied
            && selectionMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!SelectionAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang selection assertion rejected native selected state or changed focus");
        }
        var selectionMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertSelection,
            NodeId = initialSelectionWaitNodeId,
            State = UiSelectionState.Selected,
        });
        SelectionMismatchRejected = !selectionMismatch.Applied
            && selectionMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetSelectionMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!SelectionMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang selection assertion accepted mismatched native selection or changed focus");
        }
        var missing = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = "missing-presentation-target",
        });
        if (missing.Applied
            || missing.FailureCode != PresentationAutomationFailureCode.UnknownTarget)
        {
            throw new InvalidDataException(
                "Leselang presentation focus accepted a missing target");
        }
        var nonActionNodeId = FindFirstNonActionNodeId(renderer.Document.Root)
            ?? throw new InvalidDataException("focus probe requires a non-action node");
        var selectionlessTarget = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertSelection,
            NodeId = nonActionNodeId,
            State = UiSelectionState.Selected,
        });
        SelectionlessTargetRejected = !selectionlessTarget.Applied
            && selectionlessTarget.FailureCode
                == PresentationAutomationFailureCode.SelectionlessTarget
            && renderer.FocusedNodeId == nodeId;
        if (!SelectionlessTargetRejected)
        {
            throw new InvalidDataException(
                "Leselang selection assertion accepted a selectionless target or changed focus");
        }
        var unfocusable = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = nonActionNodeId,
        });
        if (unfocusable.Applied
            || unfocusable.FailureCode != PresentationAutomationFailureCode.UnfocusableTarget)
        {
            throw new InvalidDataException(
                "Leselang presentation focus accepted an unfocusable target");
        }
        var missingNavigation = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.NavigateFocus,
            NodeId = "missing-presentation-target",
            Direction = UiFocusNavigationDirection.Next,
        });
        var unfocusableNavigation = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.NavigateFocus,
            NodeId = nonActionNodeId,
            Direction = UiFocusNavigationDirection.Next,
        });
        FocusNavigationFailuresPreservedFocus =
            !missingNavigation.Applied
            && missingNavigation.FailureCode
                == PresentationAutomationFailureCode.UnknownTarget
            && !unfocusableNavigation.Applied
            && unfocusableNavigation.FailureCode
                == PresentationAutomationFailureCode.UnfocusableTarget
            && InitialUnrealizedNavigationRejected
            && !unfocusedNavigation.Applied
            && unfocusedNavigation.FailureCode
                == PresentationAutomationFailureCode.TargetNotFocused
            && renderer.FocusedNodeId == nodeId;
        if (!FocusNavigationFailuresPreservedFocus)
        {
            throw new InvalidDataException(
                "Leselang focus navigation failure changed focus or lost a typed failure");
        }
        var scrolled = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.ScrollIntoView,
            NodeId = nonActionNodeId,
        });
        if (!scrolled.Applied
            || scrolled.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang presentation scroll failed or changed keyboard focus");
        }
        var missingScroll = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.ScrollIntoView,
            NodeId = "missing-presentation-target",
        });
        if (missingScroll.Applied
            || missingScroll.FailureCode != PresentationAutomationFailureCode.UnknownTarget)
        {
            throw new InvalidDataException(
                "Leselang presentation scroll accepted a missing target");
        }
        var visible = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertVisible,
            NodeId = nonActionNodeId,
        });
        if (!visible.Applied
            || visible.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang visibility assertion rejected a visible target or changed focus");
        }
        var visibleMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertHidden,
            NodeId = nonActionNodeId,
        });
        VisibleMismatchRejected = !visibleMismatch.Applied
            && visibleMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetStillVisible
            && renderer.FocusedNodeId == nodeId;
        if (!VisibleMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang hidden assertion accepted a visible target or changed focus");
        }
        var realized = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertRealized,
            NodeId = nodeId,
        });
        if (!realized.Applied
            || realized.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang realization assertion rejected a realized target or changed focus");
        }
        if (!InitialUnrealizedAssertionRejected
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang realization assertion lost its pre-layout rejection evidence or changed focus");
        }
        SelectionProbePreservedFocus = SelectionAssertCompleted
            && SelectionMismatchRejected
            && SelectionlessTargetRejected
            && renderer.FocusedNodeId == nodeId;
        if (!SelectionProbePreservedFocus)
        {
            throw new InvalidDataException(
                "Leselang selection probes changed keyboard focus or lost typed failure evidence");
        }
        renderer.Surface.IsVisible = false;
        var focusAfterExternalHide = renderer.FocusedNodeId;
        var hidden = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertVisible,
            NodeId = nonActionNodeId,
        });
        var hiddenAssert = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertHidden,
            NodeId = nonActionNodeId,
        });
        HiddenAssertCompleted = hiddenAssert.Applied
            && hiddenAssert.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == focusAfterExternalHide;
        renderer.Surface.IsVisible = true;
        if (hidden.Applied
            || hidden.FailureCode != PresentationAutomationFailureCode.TargetNotVisible
            || !HiddenAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang visibility assertion accepted a hidden target or hidden assertion rejected it");
        }
        var refocused = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = nodeId,
        });
        if (!refocused.Applied || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang presentation focus did not recover after the hidden assertion probe");
        }
        renderer.Mount(renderer.Document);
        if (!renderer.IsFocusRestorePending)
        {
            throw new InvalidDataException("focus restoration was not scheduled after mount");
        }
        return nodeId;
    }

    private static string? FindFirstNonActionNodeId(UiNode node)
    {
        if (node.Kind != UiNodeKind.Action)
        {
            return node.Id;
        }
        foreach (var child in node.Children)
        {
            if (FindFirstNonActionNodeId(child) is { } nodeId)
            {
                return nodeId;
            }
        }
        return null;
    }

    private static UiNode? FindFirstDescriptionNode(UiNode node)
    {
        if (node.Accessibility.Description is not null)
        {
            return node;
        }
        foreach (var child in node.Children)
        {
            if (FindFirstDescriptionNode(child) is { } described)
            {
                return described;
            }
        }
        return null;
    }

    private static UiNode? FindFirstTextNode(UiNode node)
    {
        if (node.Text is not null
            && node.Kind is UiNodeKind.Heading
                or UiNodeKind.Text
                or UiNodeKind.HistoryEntry
                or UiNodeKind.LogEntry
                or UiNodeKind.DebuggerFrame
                or UiNodeKind.Action)
        {
            return node;
        }
        foreach (var child in node.Children)
        {
            if (FindFirstTextNode(child) is { } match)
            {
                return match;
            }
        }
        return null;
    }

    private static string? FindOtherActionNodeId(UiNode node, string focusedNodeId)
    {
        if (node.Kind == UiNodeKind.Action
            && node.Action is not null
            && node.Id != focusedNodeId)
        {
            return node.Id;
        }
        foreach (var child in node.Children)
        {
            if (FindOtherActionNodeId(child, focusedNodeId) is { } nodeId)
            {
                return nodeId;
            }
        }
        return null;
    }

    private static ActionKind MismatchedActionKind(ActionKind kind) => kind switch
    {
        ActionKind.RuntimeInspect => ActionKind.RuntimeRefresh,
        _ => ActionKind.RuntimeInspect,
    };

    private static UiFormInputKind MismatchedInputKind(UiFormInputKind kind) => kind switch
    {
        UiFormInputKind.PathToken => UiFormInputKind.TrimmedText,
        UiFormInputKind.TrimmedText => UiFormInputKind.PathToken,
        _ => UiFormInputKind.PathToken,
    };

    public void CompleteFocusRetentionProbe(string nodeId)
    {
        if (renderer.IsFocusRestorePending || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "stable action focus was not restored after document mount");
        }
    }

    public void BeginPatchedFocusRetentionProbe(string nodeId)
    {
        if (!renderer.TryFocusNode(nodeId) || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException("patch focus probe could not focus its action node");
        }
        var source = FindNode(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException("patch focus probe action was not found");
        var revision = renderer.Document.Revision;
        renderer.Apply(new UiPatch
        {
            SchemaVersion = 1,
            FromRevision = revision,
            ToRevision = checked(revision + 1),
            Operations =
            [
                new UiPatchOperation
                {
                    Kind = PatchKind.Update,
                    Node = CloneShallow(source),
                },
            ],
        });
        if (!renderer.IsFocusRestorePending)
        {
            throw new InvalidDataException("focus restoration was not scheduled after patch");
        }
    }

    public void ProbeRemovedFocusTarget(string nodeId)
    {
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            renderer.Document,
            RendererJsonContext.Default.UiDocument);
        var withoutFocusedAction = JsonSerializer.Deserialize(
            payload,
            RendererJsonContext.Default.UiDocument)
            ?? throw new InvalidDataException("focus probe document clone failed");
        if (!RemoveNode(withoutFocusedAction.Root, nodeId))
        {
            throw new InvalidDataException("focus probe could not remove its action node");
        }
        renderer.Mount(withoutFocusedAction);
        if (renderer.IsFocusRestorePending || renderer.FocusedNodeId is not null)
        {
            throw new InvalidDataException(
                "removed action focus was transferred to another control");
        }
    }

    private Border BuildStatusBar()
    {
        var revisionText = new TextBlock
        {
            Foreground = LeserpentTheme.Primary,
            FontSize = 12,
            FontWeight = FontWeight.SemiBold,
            Text = $"UI IR v1  /  rev {Revision}",
        };
        var bar = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(24, 12),
            Child = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
                Children = { statusText, revisionText },
            },
        };
        Grid.SetColumn(revisionText, 1);
        Grid.SetRow(bar, 1);
        return bar;
    }

    private void OnActionInvoked(string nodeId)
    {
        invokedActionCount++;
        statusText.Text = $"Action node emitted: {nodeId}";
        statusText.Foreground = LeserpentTheme.Accent;
    }

    private static void RequireExpectedDocument(UiDocument actual, UiDocument expected)
    {
        if (!JsonNode.DeepEquals(
            JsonSerializer.SerializeToNode(actual, RendererJsonContext.Default.UiDocument),
            JsonSerializer.SerializeToNode(expected, RendererJsonContext.Default.UiDocument)))
        {
            throw new InvalidDataException("Avalonia patch result does not match the fixture");
        }
    }

    private static bool RemoveNode(UiNode parent, string nodeId)
    {
        var index = parent.Children.FindIndex(child => child.Id == nodeId);
        if (index >= 0)
        {
            parent.Children.RemoveAt(index);
            return true;
        }
        return parent.Children.Any(child => RemoveNode(child, nodeId));
    }

    private static UiNode? FindNode(UiNode node, string nodeId)
    {
        if (node.Id == nodeId)
        {
            return node;
        }
        return node.Children.Select(FindChild).FirstOrDefault(found => found is not null);

        UiNode? FindChild(UiNode child) => FindNode(child, nodeId);
    }

    private static UiNode CloneShallow(UiNode node) => new()
    {
        Id = node.Id,
        Kind = node.Kind,
        RuntimeId = node.RuntimeId,
        DebuggerSessionId = node.DebuggerSessionId,
        Text = node.Text is null ? null : new LocalizedText
        {
            Key = node.Text.Key,
            Fallback = node.Text.Fallback,
        },
        Accessibility = new Accessibility
        {
            Label = node.Accessibility.Label is null ? null : new LocalizedText
            {
                Key = node.Accessibility.Label.Key,
                Fallback = node.Accessibility.Label.Fallback,
            },
            Description = node.Accessibility.Description is null ? null : new LocalizedText
            {
                Key = node.Accessibility.Description.Key,
                Fallback = node.Accessibility.Description.Fallback,
            },
        },
        Selection = node.Selection is null ? null : new UiSelection
        {
            State = node.Selection.State,
        },
        Action = node.Action is null ? null : new UiAction
        {
            Kind = node.Action.Kind,
            RuntimeId = node.Action.RuntimeId,
            SessionId = node.Action.SessionId,
        },
        Children = [],
    };
}
