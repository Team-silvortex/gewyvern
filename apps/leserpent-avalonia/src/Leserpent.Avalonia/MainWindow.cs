using System.Text.Json;
using System.Text.Json.Nodes;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class MainWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly bool presentationProbesEnabled;
    private readonly Task<PresentationAutomationResult> initialRealizedWait = null!;
    private readonly Task<PresentationAutomationResult> initialRealizedWaitTimeout = null!;
    private readonly Task<PresentationAutomationResult> initialVisibleWait = null!;
    private readonly Task<PresentationAutomationResult> initialVisibleWaitTimeout = null!;
    private readonly Task<PresentationAutomationResult> initialEnabledWaitTimeout = null!;
    private readonly Task<PresentationAutomationResult> initialSelectionWait = null!;
    private readonly Task<PresentationAutomationResult> initialSelectionWaitTimeout = null!;
    private readonly UiDocument childCountInitialDocument = null!;
    private readonly UiPatch childCountPatch = null!;
    private readonly UiPresentationOperation childCountAssertOperation = null!;
    private readonly UiPresentationOperation childCountWaitOperation = null!;
    private readonly Task<PresentationAutomationResult> initialWindowOpenWait = null!;
    private readonly PresentationAutomationResult initialWindowClosedAssert = null!;
    private readonly Task<PresentationAutomationResult> initialWindowClosedWait = null!;
    private readonly string initialEnabledWaitNodeId = string.Empty;
    private readonly string initialFocusedWaitNodeId = string.Empty;
    private readonly string initialFocusedWaitTimeoutNodeId = string.Empty;
    private readonly string initialSelectionAssertNodeId = string.Empty;
    private readonly string initialSelectionWaitNodeId = string.Empty;
    private readonly string initialWindowOpenAssertNodeId = string.Empty;
    private readonly string initialWindowOpenWaitNodeId = string.Empty;
    private readonly string initialWindowClosedAssertNodeId = string.Empty;
    private readonly string initialWindowClosedWaitNodeId = string.Empty;
    private readonly UiDocument windowLifecycleDocument = null!;
    private readonly string windowLifecycleOpenNodeId = string.Empty;
    private readonly string windowLifecycleCloseNodeId = string.Empty;
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
    public bool WindowClosedAssertCompleted { get; private set; }
    public bool WindowClosedWaitCompleted { get; private set; }
    public bool WindowClosedWaitTimedOut { get; private set; }
    public bool WindowOpenMutationCompleted { get; private set; }
    public bool WindowCloseMutationCompleted { get; private set; }
    public bool WindowReopenMutationCompleted { get; private set; }
    public bool WindowRecloseMutationCompleted { get; private set; }
    public bool WindowLifecycleIdempotent { get; private set; }
    public bool WindowLifecycleUsedFreshNativeWindow { get; private set; }
    public bool WindowLifecycleRematerializedSemanticTree { get; private set; }
    public bool WindowLifecycleStateObserved { get; private set; }
    public bool InitialFocusedWaitCompleted { get; private set; }
    public bool InitialFocusedWaitTimedOut { get; private set; }
    public bool UnfocusedAssertCompleted { get; private set; }
    public bool InitialUnfocusedWaitCompleted { get; private set; }
    public bool InitialUnfocusedWaitTimedOut { get; private set; }
    public bool InitialUnfocusedWaitObservedExternalDeactivation { get; private set; }
    public bool InitialSelectionWaitCompleted { get; private set; }
    public bool InitialSelectionWaitTimedOut { get; private set; }
    public bool ChildCountAssertCompleted { get; private set; }
    public bool InitialChildCountWaitCompleted { get; private set; }
    public bool InitialChildCountWaitTimedOut { get; private set; }
    public bool ChildCountObservationPreservedVirtualization { get; private set; }
    public bool InitialTextWaitCompleted { get; private set; }
    public bool InitialTextWaitTimedOut { get; private set; }
    public bool InitialAccessibleNameWaitCompleted { get; private set; }
    public bool InitialAccessibleNameWaitTimedOut { get; private set; }
    public bool InitialAccessibleDescriptionWaitCompleted { get; private set; }
    public bool InitialAccessibleDescriptionWaitTimedOut { get; private set; }
    public bool InitialFormFieldWaitCompleted { get; private set; }
    public bool InitialFormFieldWaitTimedOut { get; private set; }
    public bool InitialFormFieldInputKindWaitCompleted { get; private set; }
    public bool InitialFormFieldInputKindWaitTimedOut { get; private set; }
    public bool InitialFormFieldRequiredWaitCompleted { get; private set; }
    public bool InitialFormFieldRequiredWaitTimedOut { get; private set; }
    public bool InitialFormFieldMaxLengthWaitCompleted { get; private set; }
    public bool InitialFormFieldMaxLengthWaitTimedOut { get; private set; }
    public bool InitialFormFieldPlaceholderWaitCompleted { get; private set; }
    public bool InitialFormFieldPlaceholderWaitTimedOut { get; private set; }
    public bool SelectionAssertCompleted { get; private set; }
    public bool SelectionMismatchRejected { get; private set; }
    public bool SelectionlessTargetRejected { get; private set; }
    public bool SelectionProbePreservedFocus { get; private set; }
    public bool ActionActivationCompleted { get; private set; }
    public bool ActionActivationExactlyOnce { get; private set; }
    public bool UnavailableActionActivationRejected { get; private set; }
    public bool HiddenActionActivationRejected { get; private set; }
    public bool NonActionActivationRejected { get; private set; }
    public bool MissingActionActivationRejected { get; private set; }
    public bool FocusNavigationForwardCompleted { get; private set; }
    public bool FocusNavigationBackwardCompleted { get; private set; }
    public bool FocusNavigationFirstCompleted { get; private set; }
    public bool FocusNavigationLastCompleted { get; private set; }
    public bool FocusNavigationFailuresPreservedFocus { get; private set; }
    public bool FocusNavigationDidNotActivate { get; private set; }
    public bool ActionKindAssertCompleted { get; private set; }
    public bool ActionKindWaitCompleted { get; private set; }
    public bool ActionKindWaitTimedOut { get; private set; }
    public bool ActionKindMismatchRejected { get; private set; }
    public bool NodeKindWaitCompleted { get; private set; }
    public bool NodeKindWaitTimedOut { get; private set; }
    public bool ActionLabelAssertCompleted { get; private set; }
    public bool ActionLabelWaitCompleted { get; private set; }
    public bool ActionLabelWaitTimedOut { get; private set; }
    public bool ActionLabelMismatchRejected { get; private set; }
    public bool ActionAvailableAssertCompleted { get; private set; }
    public bool ActionAvailableWaitCompleted { get; private set; }
    public bool ActionAvailableWaitTimedOut { get; private set; }
    public bool ActionUnavailableReasonAssertCompleted { get; private set; }
    public bool ActionUnavailableReasonWaitCompleted { get; private set; }
    public bool ActionUnavailableReasonWaitClearedCompleted { get; private set; }
    public bool ActionUnavailableReasonWaitTimedOut { get; private set; }
    public bool ActionUnavailableReasonMismatchRejected { get; private set; }
    public bool FormFieldAssertCompleted { get; private set; }
    public bool FormFieldMismatchRejected { get; private set; }
    public bool FormFieldInputKindAssertCompleted { get; private set; }
    public bool FormFieldInputKindMismatchRejected { get; private set; }
    public bool FormFieldRequiredAssertCompleted { get; private set; }
    public bool FormFieldRequiredMismatchRejected { get; private set; }
    public bool FormFieldMaxLengthAssertCompleted { get; private set; }
    public bool FormFieldMaxLengthMismatchRejected { get; private set; }
    public bool FormFieldPlaceholderAssertCompleted { get; private set; }
    public bool FormFieldPlaceholderMismatchRejected { get; private set; }
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

    public MainWindow(RendererFixture fixture, bool presentationProbesEnabled)
    {
        this.presentationProbesEnabled = presentationProbesEnabled;
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
        if (!presentationProbesEnabled)
        {
            ConfigureWindowContent();
            return;
        }
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
        initialEnabledWaitNodeId = fixture.EnabledWaitOperation?.NodeId
            ?? throw new InvalidDataException(
                "enabled wait probe requires an action target");
        renderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            false,
            "Verification action is temporarily unavailable");
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
                NodeId = initialEnabledWaitNodeId,
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
        initialWindowClosedAssertNodeId = fixture.WindowClosedAssertOperation?.NodeId
            ?? throw new InvalidDataException(
                "window-closed assertion probe requires a semantic target");
        initialWindowClosedWaitNodeId = fixture.WindowClosedWaitOperation?.NodeId
            ?? throw new InvalidDataException(
                "window-closed wait probe requires a semantic target");
        var windowClosedDetachedRenderer = new AvaloniaDocumentRenderer(_ => { });
        windowClosedDetachedRenderer.Mount(fixture.Next);
        if (!windowClosedDetachedRenderer.RealizeNodeForVerification(initialWindowClosedAssertNodeId)
            || !windowClosedDetachedRenderer.RealizeNodeForVerification(initialWindowClosedWaitNodeId))
        {
            throw new InvalidDataException(
                "window-closed probe requires a detached realized semantic target");
        }
        initialWindowClosedAssert = windowClosedDetachedRenderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertWindowClosed,
                NodeId = initialWindowClosedAssertNodeId,
            });
        initialWindowClosedWait = windowClosedDetachedRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitWindowClosed,
                NodeId = initialWindowClosedWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitWindowClosedTimeoutMs,
            });
        windowLifecycleDocument = fixture.Next;
        windowLifecycleOpenNodeId = fixture.WindowOpenOperation?.NodeId
            ?? throw new InvalidDataException(
                "window-open mutation probe requires a semantic target");
        windowLifecycleCloseNodeId = fixture.WindowCloseOperation?.NodeId
            ?? throw new InvalidDataException(
                "window-close mutation probe requires a semantic target");
        initialSelectionWaitTimeout = detachedRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitSelection,
                NodeId = initialSelectionWaitNodeId,
                State = UiSelectionState.Selected,
                TimeoutMs = SemanticRenderer.WaitSelectionTimeoutMs,
            });
        childCountInitialDocument = fixture.Previous;
        childCountPatch = fixture.Patch;
        childCountAssertOperation = fixture.ChildCountAssertOperation
            ?? throw new InvalidDataException(
                "child-count assertion probe requires a semantic target");
        childCountWaitOperation = fixture.ChildCountWaitOperation
            ?? throw new InvalidDataException(
                "child-count wait probe requires a semantic target");
        ConfigureWindowContent();
    }

    public async Task CompleteInitialWaitProbesAsync()
    {
        RequirePresentationProbes();
        const string unavailableReason = "Verification action is temporarily unavailable";
        var childCountRenderer = new AvaloniaDocumentRenderer(_ => { });
        childCountRenderer.Mount(childCountInitialDocument);
        var unrealizedBeforeAssert = childCountRenderer.UnrealizedNodeCount;
        var initialChildCount = childCountRenderer.ApplyPresentation(childCountAssertOperation);
        var unrealizedAfterAssert = childCountRenderer.UnrealizedNodeCount;
        var childCountWait = childCountRenderer.ApplyPresentationAsync(childCountWaitOperation);
        DispatcherTimer.RunOnce(
            () => childCountRenderer.Apply(childCountPatch),
            TimeSpan.FromMilliseconds(50));
        var childCountWaitResult = await childCountWait;
        var patchedChildCount = childCountRenderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertChildCount,
                NodeId = childCountWaitOperation.NodeId,
                Count = childCountWaitOperation.Count,
            });
        var unrealizedBeforeTimeout = childCountRenderer.UnrealizedNodeCount;
        var childCountTimeout = await childCountRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitChildCount,
                NodeId = childCountAssertOperation.NodeId,
                Count = childCountAssertOperation.Count,
                TimeoutMs = SemanticRenderer.WaitChildCountTimeoutMs,
            });
        var unrealizedAfterTimeout = childCountRenderer.UnrealizedNodeCount;
        ChildCountAssertCompleted = initialChildCount.Applied
            && initialChildCount.FailureCode == PresentationAutomationFailureCode.None
            && patchedChildCount.Applied
            && patchedChildCount.FailureCode == PresentationAutomationFailureCode.None;
        InitialChildCountWaitCompleted = childCountWaitResult.Applied
            && childCountWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        InitialChildCountWaitTimedOut = !childCountTimeout.Applied
            && childCountTimeout.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        ChildCountObservationPreservedVirtualization =
            unrealizedBeforeAssert == unrealizedAfterAssert
            && unrealizedBeforeTimeout == unrealizedAfterTimeout;
        if (!ChildCountAssertCompleted
            || !InitialChildCountWaitCompleted
            || !InitialChildCountWaitTimedOut
            || !ChildCountObservationPreservedVirtualization)
        {
            throw new InvalidDataException(
                "Leselang child-count observation diverged across an external patch: "
                + $"asserted={ChildCountAssertCompleted}, "
                + $"waited={InitialChildCountWaitCompleted}, "
                + $"timed_out={InitialChildCountWaitTimedOut}, "
                + $"virtualization_preserved={ChildCountObservationPreservedVirtualization}");
        }
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
        WindowClosedAssertCompleted = initialWindowClosedAssert.Applied
            && initialWindowClosedAssert.FailureCode
                == PresentationAutomationFailureCode.None;
        if (!WindowClosedAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang window-closed assertion did not observe the detached native surface");
        }
        var windowClosedWait = await initialWindowClosedWait;
        WindowClosedWaitCompleted = windowClosedWait.Applied
            && windowClosedWait.FailureCode == PresentationAutomationFailureCode.None;
        if (!WindowClosedWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang window-closed wait did not observe the detached native surface");
        }
        var windowClosedTimeout = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitWindowClosed,
                NodeId = initialWindowClosedWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitWindowClosedTimeoutMs,
            });
        var stillOpen = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertWindowOpen,
            NodeId = initialWindowClosedWaitNodeId,
        });
        WindowClosedWaitTimedOut = !windowClosedTimeout.Applied
            && windowClosedTimeout.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && stillOpen.Applied
            && stillOpen.FailureCode == PresentationAutomationFailureCode.None;
        if (!WindowClosedWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang window-closed wait closed an open window or failed to time out: "
                + $"result_applied={windowClosedTimeout.Applied}, "
                + $"result_failure={windowClosedTimeout.FailureCode}, "
                + $"still_open={stillOpen.Applied}");
        }
        var lifecycleRenderer = new AvaloniaDocumentRenderer(_ => { });
        lifecycleRenderer.Mount(windowLifecycleDocument);
        if (!lifecycleRenderer.RealizeNodeForVerification(windowLifecycleOpenNodeId)
            || !lifecycleRenderer.RealizeNodeForVerification(windowLifecycleCloseNodeId))
        {
            throw new InvalidDataException(
                "window lifecycle mutation probe requires realized semantic targets");
        }
        var opened = lifecycleRenderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.OpenWindow,
            NodeId = windowLifecycleOpenNodeId,
        });
        await Dispatcher.UIThread.InvokeAsync(() => { });
        var observedOpen = lifecycleRenderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertWindowOpen,
            NodeId = windowLifecycleOpenNodeId,
        });
        var closed = lifecycleRenderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.CloseWindow,
            NodeId = windowLifecycleCloseNodeId,
        });
        await Dispatcher.UIThread.InvokeAsync(() => { });
        var observedClosed = lifecycleRenderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertWindowClosed,
            NodeId = windowLifecycleCloseNodeId,
        });
        var reopened = lifecycleRenderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.OpenWindow,
            NodeId = windowLifecycleOpenNodeId,
        });
        await Dispatcher.UIThread.InvokeAsync(() => { });
        var observedReopened = lifecycleRenderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertWindowOpen,
                NodeId = windowLifecycleOpenNodeId,
            });
        var duplicateOpen = lifecycleRenderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.OpenWindow,
                NodeId = windowLifecycleOpenNodeId,
            });
        var generationCountAfterDuplicateOpen =
            lifecycleRenderer.PresentationWindowGenerationCount;
        var reclosed = lifecycleRenderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.CloseWindow,
            NodeId = windowLifecycleCloseNodeId,
        });
        await Dispatcher.UIThread.InvokeAsync(() => { });
        var observedReclosed = lifecycleRenderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertWindowClosed,
                NodeId = windowLifecycleCloseNodeId,
            });
        var duplicateClose = lifecycleRenderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.CloseWindow,
                NodeId = windowLifecycleCloseNodeId,
            });
        var observedDuplicateClose = lifecycleRenderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertWindowClosed,
                NodeId = windowLifecycleCloseNodeId,
            });
        WindowOpenMutationCompleted = AppliedWithoutFailure(opened);
        WindowCloseMutationCompleted = AppliedWithoutFailure(closed);
        WindowReopenMutationCompleted = AppliedWithoutFailure(reopened);
        WindowRecloseMutationCompleted = AppliedWithoutFailure(reclosed);
        WindowLifecycleIdempotent = AppliedWithoutFailure(duplicateOpen)
            && AppliedWithoutFailure(duplicateClose)
            && AppliedWithoutFailure(observedDuplicateClose)
            && generationCountAfterDuplicateOpen == 2;
        WindowLifecycleUsedFreshNativeWindow =
            lifecycleRenderer.PresentationWindowGenerationCount == 2;
        WindowLifecycleRematerializedSemanticTree =
            lifecycleRenderer.PresentationTreeRematerializationCount == 2;
        WindowLifecycleStateObserved = AppliedWithoutFailure(observedOpen)
            && AppliedWithoutFailure(observedClosed)
            && AppliedWithoutFailure(observedReopened)
            && AppliedWithoutFailure(observedReclosed);
        if (!WindowOpenMutationCompleted
            || !WindowCloseMutationCompleted
            || !WindowReopenMutationCompleted
            || !WindowRecloseMutationCompleted
            || !WindowLifecycleIdempotent
            || !WindowLifecycleUsedFreshNativeWindow
            || !WindowLifecycleRematerializedSemanticTree
            || !WindowLifecycleStateObserved)
        {
            throw new InvalidDataException(
                "Leselang window lifecycle mutations diverged from native window state: "
                + $"opened={WindowOpenMutationCompleted}, "
                + $"closed={WindowCloseMutationCompleted}, "
                + $"reopened={WindowReopenMutationCompleted}, "
                + $"reclosed={WindowRecloseMutationCompleted}, "
                + $"idempotent={WindowLifecycleIdempotent}, "
                + $"fresh_native_window={WindowLifecycleUsedFreshNativeWindow}, "
                + $"semantic_tree_rematerialized={WindowLifecycleRematerializedSemanticTree}, "
                + $"state_observed={WindowLifecycleStateObserved}");
        }
        var initiallyDisabled = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertDisabled,
            NodeId = initialEnabledWaitNodeId,
        });
        if (!initiallyDisabled.Applied
            || initiallyDisabled.FailureCode != PresentationAutomationFailureCode.None)
        {
            throw new InvalidDataException(
                "Leselang enabled wait probe target did not start disabled");
        }
        var enabledWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitEnabled,
            NodeId = initialEnabledWaitNodeId,
            TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => renderer.SetActionAvailability(
                ActionKind.RuntimeRefresh,
                true,
                null),
            TimeSpan.FromMilliseconds(50));
        var focusedWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFocused,
                NodeId = initialFocusedWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitFocusedTimeoutMs,
            });
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
        var enabledResult = await enabledWait;
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
                unavailableReason),
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
        var actionLabelProbeNode = FindNode(renderer.Document.Root, enabledResult.NodeId)
            ?? throw new InvalidDataException("action label wait probe target was not found");
        var expectedActionLabel = actionLabelProbeNode.Accessibility.Label?.Fallback
            ?? throw new InvalidDataException("action label wait probe requires an explicit label");
        var actionLabelWaitMatched = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionLabel,
                NodeId = enabledResult.NodeId,
                Expected = expectedActionLabel,
                TimeoutMs = SemanticRenderer.WaitActionLabelTimeoutMs,
            });
        ActionLabelWaitCompleted = actionLabelWaitMatched.Applied
            && actionLabelWaitMatched.FailureCode == PresentationAutomationFailureCode.None;
        if (!ActionLabelWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang action label wait rejected the stable semantic action label");
        }
        var actionLabelWaitTimeout = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionLabel,
                NodeId = enabledResult.NodeId,
                Expected = $"{expectedActionLabel} mismatch",
                TimeoutMs = SemanticRenderer.WaitActionLabelTimeoutMs,
            });
        ActionLabelWaitTimedOut = !actionLabelWaitTimeout.Applied
            && actionLabelWaitTimeout.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        if (!ActionLabelWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang action label wait did not reject a persistent semantic label mismatch");
        }
        var nodeKindWaitMatched = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitNodeKind,
                NodeId = enabledResult.NodeId,
                ExpectedKind = actionLabelProbeNode.Kind,
                TimeoutMs = SemanticRenderer.WaitNodeKindTimeoutMs,
            });
        NodeKindWaitCompleted = nodeKindWaitMatched.Applied
            && nodeKindWaitMatched.FailureCode == PresentationAutomationFailureCode.None;
        if (!NodeKindWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang node kind wait rejected the stable semantic node kind");
        }
        var mismatchedNodeKind = actionLabelProbeNode.Kind == UiNodeKind.Text
            ? UiNodeKind.Heading
            : UiNodeKind.Text;
        var nodeKindWaitTimeout = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitNodeKind,
                NodeId = enabledResult.NodeId,
                ExpectedKind = mismatchedNodeKind,
                TimeoutMs = SemanticRenderer.WaitNodeKindTimeoutMs,
            });
        NodeKindWaitTimedOut = !nodeKindWaitTimeout.Applied
            && nodeKindWaitTimeout.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        if (!NodeKindWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang node kind wait did not reject a persistent semantic kind mismatch");
        }
        var expectedActionKind = actionLabelProbeNode.Action?.Kind
            ?? throw new InvalidDataException("action kind wait probe requires an action target");
        var actionKindWaitMatched = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionKind,
                NodeId = enabledResult.NodeId,
                ExpectedActionKind = expectedActionKind,
                TimeoutMs = SemanticRenderer.WaitActionKindTimeoutMs,
            });
        ActionKindWaitCompleted = actionKindWaitMatched.Applied
            && actionKindWaitMatched.FailureCode == PresentationAutomationFailureCode.None;
        if (!ActionKindWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang action kind wait rejected the stable semantic action kind");
        }
        var actionKindWaitTimeout = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionKind,
                NodeId = enabledResult.NodeId,
                ExpectedActionKind = MismatchedActionKind(expectedActionKind),
                TimeoutMs = SemanticRenderer.WaitActionKindTimeoutMs,
            });
        ActionKindWaitTimedOut = !actionKindWaitTimeout.Applied
            && actionKindWaitTimeout.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        if (!ActionKindWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang action kind wait did not reject a persistent semantic action mismatch");
        }
        var actionAvailableAssert = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionAvailable,
            NodeId = enabledResult.NodeId,
        });
        ActionAvailableAssertCompleted = actionAvailableAssert.Applied
            && actionAvailableAssert.FailureCode == PresentationAutomationFailureCode.None;
        if (!ActionAvailableAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang action-available assertion did not observe an available action");
        }
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, false, unavailableReason);
        var actionAvailableWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionAvailable,
                NodeId = enabledResult.NodeId,
                TimeoutMs = SemanticRenderer.WaitActionAvailableTimeoutMs,
            });
        DispatcherTimer.RunOnce(
            () => renderer.SetActionAvailability(ActionKind.RuntimeRefresh, true, null),
            TimeSpan.FromMilliseconds(50));
        var actionAvailableWaitResult = await actionAvailableWait;
        ActionAvailableWaitCompleted = actionAvailableWaitResult.Applied
            && actionAvailableWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!ActionAvailableWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang action-available wait did not observe availability restoration");
        }
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, false, unavailableReason);
        var actionAvailableWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionAvailable,
                NodeId = enabledResult.NodeId,
                TimeoutMs = SemanticRenderer.WaitActionAvailableTimeoutMs,
            });
        ActionAvailableWaitTimedOut =
            !actionAvailableWaitTimeoutResult.Applied
            && actionAvailableWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
                NodeId = enabledResult.NodeId,
                Expected = unavailableReason,
            }).Applied;
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, true, null);
        if (!ActionAvailableWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang action-available wait did not reject a persistently unavailable action");
        }
        var actionUnavailableReasonWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
                NodeId = enabledResult.NodeId,
                Expected = unavailableReason,
                TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
            });
        DispatcherTimer.RunOnce(
            () => renderer.SetActionAvailability(
                ActionKind.RuntimeRefresh,
                false,
                unavailableReason),
            TimeSpan.FromMilliseconds(50));
        var actionUnavailableReasonWaitResult = await actionUnavailableReasonWait;
        ActionUnavailableReasonWaitCompleted = actionUnavailableReasonWaitResult.Applied
            && actionUnavailableReasonWaitResult.FailureCode
                == PresentationAutomationFailureCode.None;
        if (!ActionUnavailableReasonWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang action-unavailable-reason wait did not observe an external reason transition");
        }
        var actionUnavailableReasonClearWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
                NodeId = enabledResult.NodeId,
                TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
            });
        DispatcherTimer.RunOnce(
            () => renderer.SetActionAvailability(ActionKind.RuntimeRefresh, true, null),
            TimeSpan.FromMilliseconds(50));
        var actionUnavailableReasonClearResult = await actionUnavailableReasonClearWait;
        ActionUnavailableReasonWaitClearedCompleted =
            actionUnavailableReasonClearResult.Applied
            && actionUnavailableReasonClearResult.FailureCode
                == PresentationAutomationFailureCode.None;
        if (!ActionUnavailableReasonWaitClearedCompleted)
        {
            throw new InvalidDataException(
                "Leselang action-unavailable-reason wait did not observe reason clearing");
        }
        var actionUnavailableReasonTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
                NodeId = enabledResult.NodeId,
                Expected = unavailableReason,
                TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
            });
        ActionUnavailableReasonWaitTimedOut =
            !actionUnavailableReasonTimeoutResult.Applied
            && actionUnavailableReasonTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
                NodeId = enabledResult.NodeId,
            }).Applied;
        if (!ActionUnavailableReasonWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang action-unavailable-reason wait did not reject a persistent mismatch");
        }
        var focusedResult = await focusedWait;
        InitialFocusedWaitCompleted = focusedResult.Applied
            && focusedResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialFocusedWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang focused wait did not observe an external focus transition");
        }
        var unfocusedAssertBaseline = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.Focus,
                NodeId = initialFocusedWaitNodeId,
            });
        await Dispatcher.UIThread.InvokeAsync(() => { });
        if (!unfocusedAssertBaseline.Applied
            || unfocusedAssertBaseline.FailureCode
                != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != initialFocusedWaitNodeId)
        {
            throw new InvalidDataException(
                "Leselang unfocused assertion probe could not establish its focus baseline");
        }
        var unfocusedAssertResult = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertUnfocused,
                NodeId = initialFocusedWaitTimeoutNodeId,
            });
        UnfocusedAssertCompleted = unfocusedAssertResult.Applied
            && unfocusedAssertResult.FailureCode
                == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == initialFocusedWaitNodeId;
        if (!UnfocusedAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang unfocused assertion rejected an unfocused action or changed focus");
        }
        var unfocusedWait = renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitUnfocused,
                NodeId = initialFocusedWaitNodeId,
                TimeoutMs = SemanticRenderer.WaitUnfocusedTimeoutMs,
            });
        DispatcherTimer.RunOnce(
            () =>
            {
                var focused = renderer.ApplyPresentation(new UiPresentationOperation
                {
                    Kind = UiPresentationOperationKind.Focus,
                    NodeId = initialFocusedWaitTimeoutNodeId,
                });
                if (!focused.Applied
                    || focused.FailureCode
                        != PresentationAutomationFailureCode.None)
                {
                    throw new InvalidDataException(
                        "unfocused wait probe could not apply its external focus transition");
                }
            },
            TimeSpan.FromMilliseconds(50));
        var unfocusedWaitResult = await unfocusedWait;
        InitialUnfocusedWaitCompleted = unfocusedWaitResult.Applied
            && unfocusedWaitResult.FailureCode
                == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == initialFocusedWaitTimeoutNodeId;
        if (!InitialUnfocusedWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang unfocused wait did not observe an external focus transition");
        }
        Activate();
        var unfocusedTimeoutBaseline = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.Focus,
                NodeId = initialFocusedWaitTimeoutNodeId,
            });
        await Dispatcher.UIThread.InvokeAsync(() => { });
        if (!unfocusedTimeoutBaseline.Applied
            || unfocusedTimeoutBaseline.FailureCode
                != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != initialFocusedWaitTimeoutNodeId)
        {
            throw new InvalidDataException(
                "Leselang unfocused timeout probe could not establish its focus baseline");
        }
        var unfocusedTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitUnfocused,
                NodeId = initialFocusedWaitTimeoutNodeId,
                TimeoutMs = SemanticRenderer.WaitUnfocusedTimeoutMs,
            });
        var persistentFocus = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertFocused,
                NodeId = initialFocusedWaitTimeoutNodeId,
            });
        InitialUnfocusedWaitTimedOut = !unfocusedTimeoutResult.Applied
            && unfocusedTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && persistentFocus.Applied
            && persistentFocus.FailureCode
                == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == initialFocusedWaitTimeoutNodeId;
        InitialUnfocusedWaitObservedExternalDeactivation =
            unfocusedTimeoutResult.Applied
            && unfocusedTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.None
            && !IsActive
            && renderer.FocusedNodeId is null;
        if (!InitialUnfocusedWaitTimedOut
            && !InitialUnfocusedWaitObservedExternalDeactivation)
        {
            throw new InvalidDataException(
                "Leselang unfocused wait neither timed out on persistent focus nor observed native window deactivation: "
                + $"result_applied={unfocusedTimeoutResult.Applied}, "
                + $"result_failure={unfocusedTimeoutResult.FailureCode}, "
                + $"window_active={IsActive}, "
                + $"focused_node={renderer.FocusedNodeId ?? "<none>"}, "
                + $"expected_focus={initialFocusedWaitTimeoutNodeId}");
        }
        var restoredFocusedBaseline = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.Focus,
                NodeId = initialFocusedWaitNodeId,
            });
        if (!restoredFocusedBaseline.Applied
            || restoredFocusedBaseline.FailureCode
                != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != initialFocusedWaitNodeId)
        {
            throw new InvalidDataException(
                "Leselang focused wait probe could not restore its focus baseline");
        }
        var focusedTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFocused,
                NodeId = initialFocusedWaitTimeoutNodeId,
                TimeoutMs = SemanticRenderer.WaitFocusedTimeoutMs,
            });
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
                "Leselang focused wait changed focus or did not reject a persistently unfocused realized target: "
                + $"result_applied={focusedTimeoutResult.Applied}, "
                + $"result_failure={focusedTimeoutResult.FailureCode}, "
                + $"timeout_target_realized={timeoutTargetRealized.Applied}, "
                + $"timeout_target_focus_failure={timeoutTargetFocused.FailureCode}, "
                + $"focused_node={renderer.FocusedNodeId ?? "<none>"}, "
                + $"expected_focus={initialFocusedWaitNodeId}, "
                + $"timeout_target={initialFocusedWaitTimeoutNodeId}");
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
        var textWaitNode = FindFirstTextNode(renderer.Document.Root)
            ?? throw new InvalidDataException("text wait probe requires a text leaf");
        var textWaitExpected = $"{textWaitNode.Text!.Fallback} ready";
        var textWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitText,
            NodeId = textWaitNode.Id,
            Expected = textWaitExpected,
            TimeoutMs = SemanticRenderer.WaitTextTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchTextFallback(textWaitNode.Id, textWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var textWaitResult = await textWait;
        InitialTextWaitCompleted = textWaitResult.Applied
            && textWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialTextWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang text wait did not observe an external text transition");
        }
        var textWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitText,
                NodeId = textWaitNode.Id,
                Expected = $"{textWaitExpected} mismatch",
                TimeoutMs = SemanticRenderer.WaitTextTimeoutMs,
            });
        InitialTextWaitTimedOut = !textWaitTimeoutResult.Applied
            && textWaitTimeoutResult.FailureCode == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertText,
                NodeId = textWaitNode.Id,
                Expected = textWaitExpected,
            }).Applied;
        if (!InitialTextWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang text wait did not reject a persistent text mismatch");
        }
        var accessibleNameWaitExpected = $"{textWaitExpected} accessible";
        var accessibleNameWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitAccessibleName,
            NodeId = textWaitNode.Id,
            Expected = accessibleNameWaitExpected,
            TimeoutMs = SemanticRenderer.WaitAccessibleNameTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchTextFallback(textWaitNode.Id, accessibleNameWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var accessibleNameWaitResult = await accessibleNameWait;
        InitialAccessibleNameWaitCompleted = accessibleNameWaitResult.Applied
            && accessibleNameWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialAccessibleNameWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang accessible-name wait did not observe an external automation-name transition");
        }
        var accessibleNameWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitAccessibleName,
                NodeId = textWaitNode.Id,
                Expected = $"{accessibleNameWaitExpected} mismatch",
                TimeoutMs = SemanticRenderer.WaitAccessibleNameTimeoutMs,
            });
        InitialAccessibleNameWaitTimedOut = !accessibleNameWaitTimeoutResult.Applied
            && accessibleNameWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertAccessibleName,
                NodeId = textWaitNode.Id,
                Expected = accessibleNameWaitExpected,
            }).Applied;
        if (!InitialAccessibleNameWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang accessible-name wait did not reject a persistent automation-name mismatch");
        }
        var accessibleDescriptionWaitNode = FindFirstDescriptionNode(renderer.Document.Root)
            ?? throw new InvalidDataException(
                "accessible description wait probe requires explicit semantic metadata");
        var accessibleDescriptionWaitExpected =
            $"{accessibleDescriptionWaitNode.Accessibility.Description!.Fallback} ready";
        var accessibleDescriptionWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitAccessibleDescription,
            NodeId = accessibleDescriptionWaitNode.Id,
            Expected = accessibleDescriptionWaitExpected,
            TimeoutMs = SemanticRenderer.WaitAccessibleDescriptionTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchAccessibilityDescriptionFallback(
                accessibleDescriptionWaitNode.Id,
                accessibleDescriptionWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var accessibleDescriptionWaitResult = await accessibleDescriptionWait;
        InitialAccessibleDescriptionWaitCompleted = accessibleDescriptionWaitResult.Applied
            && accessibleDescriptionWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialAccessibleDescriptionWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang accessible-description wait did not observe an external automation-description transition");
        }
        var accessibleDescriptionWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitAccessibleDescription,
                NodeId = accessibleDescriptionWaitNode.Id,
                Expected = $"{accessibleDescriptionWaitExpected} mismatch",
                TimeoutMs = SemanticRenderer.WaitAccessibleDescriptionTimeoutMs,
            });
        InitialAccessibleDescriptionWaitTimedOut = !accessibleDescriptionWaitTimeoutResult.Applied
            && accessibleDescriptionWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertAccessibleDescription,
                NodeId = accessibleDescriptionWaitNode.Id,
                Expected = accessibleDescriptionWaitExpected,
            }).Applied;
        if (!InitialAccessibleDescriptionWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang accessible-description wait did not reject a persistent automation-description mismatch");
        }
        var formFieldPlaceholderWaitNodeId =
            renderer.FirstRealizedActionNodeIdFor(ActionKind.RuntimeDeploy)
            ?? throw new InvalidDataException(
                "form field placeholder wait probe requires a realized deploy action");
        var formFieldPlaceholderWaitNode =
            FindNode(renderer.Document.Root, formFieldPlaceholderWaitNodeId)
            ?? throw new InvalidDataException(
                "form field placeholder wait probe target was not found");
        var formFieldPlaceholderWaitField =
            formFieldPlaceholderWaitNode.Action?.Form?.Fields
                .FirstOrDefault(field => field.Placeholder is not null)
            ?? throw new InvalidDataException(
                "form field placeholder wait probe requires placeholder metadata");
        var formFieldWaitExpected =
            $"{formFieldPlaceholderWaitField.Label.Fallback} ready";
        var formFieldWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitFormField,
            NodeId = formFieldPlaceholderWaitNodeId,
            Field = formFieldPlaceholderWaitField.Key,
            Expected = formFieldWaitExpected,
            TimeoutMs = SemanticRenderer.WaitFormFieldTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchFormFieldLabel(
                formFieldPlaceholderWaitNodeId,
                formFieldPlaceholderWaitField.Key,
                formFieldWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var formFieldWaitResult = await formFieldWait;
        InitialFormFieldWaitCompleted = formFieldWaitResult.Applied
            && formFieldWaitResult.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialFormFieldWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field wait did not observe an external label metadata transition");
        }
        var formFieldWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFormField,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                Expected = $"{formFieldWaitExpected} mismatch",
                TimeoutMs = SemanticRenderer.WaitFormFieldTimeoutMs,
            });
        InitialFormFieldWaitTimedOut = !formFieldWaitTimeoutResult.Applied
            && formFieldWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertFormField,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                Expected = formFieldWaitExpected,
            }).Applied;
        if (!InitialFormFieldWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang form field wait did not reject a persistent label mismatch");
        }
        var formFieldInputKindWaitExpected =
            MismatchedInputKind(formFieldPlaceholderWaitField.InputKind);
        var formFieldInputKindWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitFormFieldInputKind,
            NodeId = formFieldPlaceholderWaitNodeId,
            Field = formFieldPlaceholderWaitField.Key,
            InputKind = formFieldInputKindWaitExpected,
            TimeoutMs = SemanticRenderer.WaitFormFieldInputKindTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchFormFieldInputKind(
                formFieldPlaceholderWaitNodeId,
                formFieldPlaceholderWaitField.Key,
                formFieldInputKindWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var formFieldInputKindWaitResult = await formFieldInputKindWait;
        InitialFormFieldInputKindWaitCompleted = formFieldInputKindWaitResult.Applied
            && formFieldInputKindWaitResult.FailureCode
                == PresentationAutomationFailureCode.None;
        if (!InitialFormFieldInputKindWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field input kind wait did not observe an external metadata transition");
        }
        var formFieldInputKindWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFormFieldInputKind,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                InputKind = formFieldPlaceholderWaitField.InputKind,
                TimeoutMs = SemanticRenderer.WaitFormFieldInputKindTimeoutMs,
            });
        InitialFormFieldInputKindWaitTimedOut =
            !formFieldInputKindWaitTimeoutResult.Applied
            && formFieldInputKindWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                InputKind = formFieldInputKindWaitExpected,
            }).Applied;
        if (!InitialFormFieldInputKindWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang form field input kind wait did not reject a persistent metadata mismatch");
        }
        var formFieldRequiredWaitExpected = !formFieldPlaceholderWaitField.Required;
        var formFieldRequiredWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitFormFieldRequired,
            NodeId = formFieldPlaceholderWaitNodeId,
            Field = formFieldPlaceholderWaitField.Key,
            Required = formFieldRequiredWaitExpected,
            TimeoutMs = SemanticRenderer.WaitFormFieldRequiredTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchFormFieldRequired(
                formFieldPlaceholderWaitNodeId,
                formFieldPlaceholderWaitField.Key,
                formFieldRequiredWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var formFieldRequiredWaitResult = await formFieldRequiredWait;
        InitialFormFieldRequiredWaitCompleted = formFieldRequiredWaitResult.Applied
            && formFieldRequiredWaitResult.FailureCode
                == PresentationAutomationFailureCode.None;
        if (!InitialFormFieldRequiredWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field required wait did not observe an external metadata transition");
        }
        var formFieldRequiredWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFormFieldRequired,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                Required = formFieldPlaceholderWaitField.Required,
                TimeoutMs = SemanticRenderer.WaitFormFieldRequiredTimeoutMs,
            });
        InitialFormFieldRequiredWaitTimedOut = !formFieldRequiredWaitTimeoutResult.Applied
            && formFieldRequiredWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertFormFieldRequired,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                Required = formFieldRequiredWaitExpected,
            }).Applied;
        if (!InitialFormFieldRequiredWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang form field required wait did not reject a persistent metadata mismatch");
        }
        var formFieldMaxLengthWaitExpected =
            formFieldPlaceholderWaitField.MaxLength == 1
                ? 2
                : formFieldPlaceholderWaitField.MaxLength - 1;
        var formFieldMaxLengthWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitFormFieldMaxLength,
            NodeId = formFieldPlaceholderWaitNodeId,
            Field = formFieldPlaceholderWaitField.Key,
            MaxLength = formFieldMaxLengthWaitExpected,
            TimeoutMs = SemanticRenderer.WaitFormFieldMaxLengthTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchFormFieldMaxLength(
                formFieldPlaceholderWaitNodeId,
                formFieldPlaceholderWaitField.Key,
                formFieldMaxLengthWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var formFieldMaxLengthWaitResult = await formFieldMaxLengthWait;
        InitialFormFieldMaxLengthWaitCompleted = formFieldMaxLengthWaitResult.Applied
            && formFieldMaxLengthWaitResult.FailureCode
                == PresentationAutomationFailureCode.None;
        if (!InitialFormFieldMaxLengthWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field max length wait did not observe an external metadata transition");
        }
        var formFieldMaxLengthWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFormFieldMaxLength,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                MaxLength = formFieldPlaceholderWaitField.MaxLength,
                TimeoutMs = SemanticRenderer.WaitFormFieldMaxLengthTimeoutMs,
            });
        InitialFormFieldMaxLengthWaitTimedOut =
            !formFieldMaxLengthWaitTimeoutResult.Applied
            && formFieldMaxLengthWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                MaxLength = formFieldMaxLengthWaitExpected,
            }).Applied;
        if (!InitialFormFieldMaxLengthWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang form field max length wait did not reject a persistent metadata mismatch");
        }
        var formFieldPlaceholderWaitExpected =
            $"{formFieldPlaceholderWaitField.Placeholder!.Fallback} ready";
        var formFieldPlaceholderWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
            NodeId = formFieldPlaceholderWaitNodeId,
            Field = formFieldPlaceholderWaitField.Key,
            Expected = formFieldPlaceholderWaitExpected,
            TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
        });
        DispatcherTimer.RunOnce(
            () => PatchFormFieldPlaceholder(
                formFieldPlaceholderWaitNodeId,
                formFieldPlaceholderWaitField.Key,
                formFieldPlaceholderWaitExpected),
            TimeSpan.FromMilliseconds(50));
        var formFieldPlaceholderWaitResult = await formFieldPlaceholderWait;
        InitialFormFieldPlaceholderWaitCompleted = formFieldPlaceholderWaitResult.Applied
            && formFieldPlaceholderWaitResult.FailureCode
                == PresentationAutomationFailureCode.None;
        if (!InitialFormFieldPlaceholderWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field placeholder wait did not observe an external form metadata transition");
        }
        var formFieldPlaceholderWaitTimeoutResult = await renderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                Expected = $"{formFieldPlaceholderWaitExpected} mismatch",
                TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
            });
        InitialFormFieldPlaceholderWaitTimedOut =
            !formFieldPlaceholderWaitTimeoutResult.Applied
            && formFieldPlaceholderWaitTimeoutResult.FailureCode
                == PresentationAutomationFailureCode.WaitTimedOut
            && renderer.ApplyPresentation(new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
                NodeId = formFieldPlaceholderWaitNodeId,
                Field = formFieldPlaceholderWaitField.Key,
                Expected = formFieldPlaceholderWaitExpected,
            }).Applied;
        if (!InitialFormFieldPlaceholderWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang form field placeholder wait did not reject a persistent placeholder mismatch");
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
        await Dispatcher.UIThread.InvokeAsync(
            () =>
            {
                renderer.Surface.IsVisible = true;
                renderer.Surface.InvalidateMeasure();
                renderer.Surface.InvalidateArrange();
            },
            DispatcherPriority.Render);
        if (!InitialHiddenWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang hidden wait did not observe an external hidden transition");
        }
        var restoredVisibility = await renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitVisible,
            NodeId = visibleResult.NodeId,
            TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
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
        RequirePresentationProbes();
        var nodeId = renderer.FirstRealizedActionNodeId
            ?? throw new InvalidDataException("focus probe requires a realized action");
        var actionCountBeforeActivation = invokedActionCount;
        var activated = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Activate,
            NodeId = nodeId,
        });
        ActionActivationCompleted = activated.Applied
            && activated.FailureCode == PresentationAutomationFailureCode.None;
        ActionActivationExactlyOnce = invokedActionCount == actionCountBeforeActivation + 1;
        if (!ActionActivationCompleted || !ActionActivationExactlyOnce)
        {
            throw new InvalidDataException(
                "Leselang action activation did not traverse the native click route exactly once");
        }
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
        const string unavailableReason = "Verification action is temporarily unavailable";
        renderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            false,
            unavailableReason);
        var actionCountBeforeUnavailableActivation = invokedActionCount;
        var unavailableActivation = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Activate,
            NodeId = enabledNodeId,
        });
        UnavailableActionActivationRejected = !unavailableActivation.Applied
            && unavailableActivation.FailureCode
                == PresentationAutomationFailureCode.TargetActionUnavailable
            && invokedActionCount == actionCountBeforeUnavailableActivation;
        if (!UnavailableActionActivationRejected)
        {
            throw new InvalidDataException(
                "Leselang action activation bypassed native disabled-state fencing");
        }
        var actionUnavailableReasonMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
            NodeId = enabledNodeId,
            Expected = unavailableReason,
        });
        ActionUnavailableReasonAssertCompleted = actionUnavailableReasonMatched.Applied
            && actionUnavailableReasonMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        var actionUnavailableReasonMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
            NodeId = enabledNodeId,
            Expected = $"{unavailableReason} mismatch",
        });
        ActionUnavailableReasonMismatchRejected = !actionUnavailableReasonMismatch.Applied
            && actionUnavailableReasonMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetActionUnavailableReasonMismatch
            && renderer.FocusedNodeId == nodeId;
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
        var actionUnavailableReasonCleared = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
            NodeId = enabledNodeId,
        });
        ActionUnavailableReasonAssertCompleted = ActionUnavailableReasonAssertCompleted
            && actionUnavailableReasonCleared.Applied
            && actionUnavailableReasonCleared.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
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
            || !ActionUnavailableReasonAssertCompleted
            || !ActionUnavailableReasonMismatchRejected
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang enabled/disabled assertion accepted the wrong action state, reason, or changed focus");
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
        var expectedActionLabel = focusedActionNode.Accessibility.Label?.Fallback
            ?? throw new InvalidDataException("action label probe requires an explicit action label");
        var actionLabelMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionLabel,
            NodeId = nodeId,
            Expected = expectedActionLabel,
        });
        ActionLabelAssertCompleted = actionLabelMatched.Applied
            && actionLabelMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!ActionLabelAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang action label assertion rejected the stable semantic action label or changed focus");
        }
        var mismatchedActionLabel = $"{expectedActionLabel} mismatch";
        var actionLabelMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertActionLabel,
            NodeId = nodeId,
            Expected = mismatchedActionLabel,
        });
        ActionLabelMismatchRejected = !actionLabelMismatch.Applied
            && actionLabelMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetActionLabelMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!ActionLabelMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang action label assertion accepted a mismatched semantic action label or changed focus");
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
        var formFieldMaxLengthMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
            NodeId = formActionNodeId,
            Field = formField.Key,
            MaxLength = formField.MaxLength,
        });
        FormFieldMaxLengthAssertCompleted = formFieldMaxLengthMatched.Applied
            && formFieldMaxLengthMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldMaxLengthAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field max length assertion rejected stable form metadata or changed focus");
        }
        var mismatchedMaxLength = formField.MaxLength == 1
            ? 2
            : formField.MaxLength - 1;
        var formFieldMaxLengthMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
            NodeId = formActionNodeId,
            Field = formField.Key,
            MaxLength = mismatchedMaxLength,
        });
        FormFieldMaxLengthMismatchRejected = !formFieldMaxLengthMismatch.Applied
            && formFieldMaxLengthMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetFormFieldMaxLengthMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldMaxLengthMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang form field max length assertion accepted mismatched form metadata or changed focus");
        }
        var expectedFormFieldPlaceholder = formField.Placeholder?.Fallback;
        var formFieldPlaceholderMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
            NodeId = formActionNodeId,
            Field = formField.Key,
            Expected = expectedFormFieldPlaceholder,
        });
        FormFieldPlaceholderAssertCompleted = formFieldPlaceholderMatched.Applied
            && formFieldPlaceholderMatched.FailureCode == PresentationAutomationFailureCode.None
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldPlaceholderAssertCompleted)
        {
            throw new InvalidDataException(
                "Leselang form field placeholder assertion rejected stable form metadata or changed focus");
        }
        var formFieldPlaceholderMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
            NodeId = formActionNodeId,
            Field = formField.Key,
            Expected = expectedFormFieldPlaceholder is null
                ? "unexpected placeholder"
                : $"{expectedFormFieldPlaceholder} mismatch",
        });
        FormFieldPlaceholderMismatchRejected = !formFieldPlaceholderMismatch.Applied
            && formFieldPlaceholderMismatch.FailureCode
                == PresentationAutomationFailureCode.TargetFormFieldPlaceholderMismatch
            && renderer.FocusedNodeId == nodeId;
        if (!FormFieldPlaceholderMismatchRejected)
        {
            throw new InvalidDataException(
                "Leselang form field placeholder assertion accepted mismatched form metadata or changed focus");
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
        var actionCountBeforeMissingActivation = invokedActionCount;
        var missingActivation = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Activate,
            NodeId = "missing-presentation-target",
        });
        MissingActionActivationRejected = !missingActivation.Applied
            && missingActivation.FailureCode == PresentationAutomationFailureCode.UnknownTarget
            && invokedActionCount == actionCountBeforeMissingActivation;
        if (missing.Applied
            || missing.FailureCode != PresentationAutomationFailureCode.UnknownTarget
            || !MissingActionActivationRejected)
        {
            throw new InvalidDataException(
                "Leselang presentation focus or activation accepted a missing target");
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
        var actionCountBeforeNonActionActivation = invokedActionCount;
        var nonActionActivation = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Activate,
            NodeId = nonActionNodeId,
        });
        NonActionActivationRejected = !nonActionActivation.Applied
            && nonActionActivation.FailureCode
                == PresentationAutomationFailureCode.UnfocusableTarget
            && invokedActionCount == actionCountBeforeNonActionActivation;
        if (!NonActionActivationRejected)
        {
            throw new InvalidDataException(
                "Leselang action activation accepted a non-action target");
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
        var actionCountBeforeHiddenActivation = invokedActionCount;
        var hiddenActivation = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Activate,
            NodeId = nodeId,
        });
        HiddenActionActivationRejected = !hiddenActivation.Applied
            && hiddenActivation.FailureCode
                == PresentationAutomationFailureCode.TargetNotVisible
            && invokedActionCount == actionCountBeforeHiddenActivation;
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
        if (!HiddenActionActivationRejected
            || hidden.Applied
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

    private void PatchTextFallback(string nodeId, string fallback)
    {
        var source = FindNode(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException("text wait probe target was not found");
        var replacement = CloneShallow(source);
        if (replacement.Text is null)
        {
            throw new InvalidDataException("text wait probe target has no semantic text");
        }
        replacement.Text.Fallback = fallback;
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
                    Node = replacement,
                },
            ],
        });
    }

    private void PatchAccessibilityDescriptionFallback(string nodeId, string fallback)
    {
        var source = FindNode(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException(
                "accessible description wait probe target was not found");
        var replacement = CloneShallow(source);
        if (replacement.Accessibility.Description is null)
        {
            throw new InvalidDataException(
                "accessible description wait probe target has no semantic description");
        }
        replacement.Accessibility.Description.Fallback = fallback;
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
                    Node = replacement,
                },
            ],
        });
    }

    private void PatchFormFieldPlaceholder(string nodeId, string fieldKey, string? fallback)
    {
        PatchFormFieldMetadata(
            nodeId,
            fieldKey,
            field =>
            {
                field.Placeholder = fallback is null
                    ? null
                    : new LocalizedText
                    {
                        Key = field.Placeholder?.Key
                            ?? $"runtime.deploy.form.{fieldKey}.placeholder",
                        Fallback = fallback,
                    };
            },
            "placeholder");
    }

    private void PatchFormFieldLabel(string nodeId, string fieldKey, string fallback)
    {
        PatchFormFieldMetadata(
            nodeId,
            fieldKey,
            field => field.Label.Fallback = fallback,
            "label");
    }

    private void PatchFormFieldInputKind(
        string nodeId,
        string fieldKey,
        UiFormInputKind inputKind)
    {
        PatchFormFieldMetadata(
            nodeId,
            fieldKey,
            field => field.InputKind = inputKind,
            "input kind");
    }

    private void PatchFormFieldRequired(string nodeId, string fieldKey, bool required)
    {
        PatchFormFieldMetadata(
            nodeId,
            fieldKey,
            field => field.Required = required,
            "required");
    }

    private void PatchFormFieldMaxLength(string nodeId, string fieldKey, int maxLength)
    {
        PatchFormFieldMetadata(
            nodeId,
            fieldKey,
            field => field.MaxLength = maxLength,
            "max length");
    }

    private void PatchFormFieldMetadata(
        string nodeId,
        string fieldKey,
        Action<UiFormField> patch,
        string metadata)
    {
        var source = FindNode(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException(
                $"form field {metadata} wait probe target was not found");
        var replacement = CloneShallow(source);
        var form = replacement.Action?.Form
            ?? throw new InvalidDataException(
                $"form field {metadata} wait probe target has no form metadata");
        var field = form.Fields.FirstOrDefault(candidate =>
            StringComparer.Ordinal.Equals(candidate.Key, fieldKey))
            ?? throw new InvalidDataException(
                $"form field {metadata} wait probe field was not found");
        patch(field);
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
                    Node = replacement,
                },
            ],
        });
    }

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

    private void ConfigureWindowContent()
    {
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

    private void RequirePresentationProbes()
    {
        if (!presentationProbesEnabled)
        {
            throw new InvalidOperationException(
                "presentation probes are disabled for this verification mode");
        }
    }

    private static bool AppliedWithoutFailure(PresentationAutomationResult result) =>
        result.Applied && result.FailureCode == PresentationAutomationFailureCode.None;

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
        Text = node.Text is null ? null : CloneLocalizedText(node.Text),
        Accessibility = new Accessibility
        {
            Label = node.Accessibility.Label is null
                ? null
                : CloneLocalizedText(node.Accessibility.Label),
            Description = node.Accessibility.Description is null
                ? null
                : CloneLocalizedText(node.Accessibility.Description),
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
            Form = node.Action.Form is null ? null : CloneForm(node.Action.Form),
        },
        Children = [],
    };

    private static UiForm CloneForm(UiForm form) => new()
    {
        Title = CloneLocalizedText(form.Title),
        SubmitLabel = CloneLocalizedText(form.SubmitLabel),
        Fields = form.Fields.Select(field => new UiFormField
        {
            Key = field.Key,
            Label = CloneLocalizedText(field.Label),
            Placeholder = field.Placeholder is null
                ? null
                : CloneLocalizedText(field.Placeholder),
            Required = field.Required,
            MaxLength = field.MaxLength,
            InputKind = field.InputKind,
        }).ToList(),
    };

    private static LocalizedText CloneLocalizedText(LocalizedText text) => new()
    {
        Key = text.Key,
        Fallback = text.Fallback,
    };
}
